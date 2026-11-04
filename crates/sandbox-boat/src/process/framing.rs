//! Exact outer Boat offsets and inner trusted child-event validation.

use base64::{Engine, engine::general_purpose::STANDARD};
use sandbox_interface::ProcessStreamEvent;
use serde::Deserialize;

use crate::{
    error::{Error, Result},
    runtime::client::decode,
};

use super::lines::Lines;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(super) enum Outer {
    Attached {
        #[serde(rename = "processId")]
        process: u64,
        #[serde(rename = "stdoutOffset")]
        stdout: u64,
        #[serde(rename = "stderrOffset")]
        stderr: u64,
    },
    Stdout {
        data: String,
        #[serde(rename = "nextOffset")]
        next: u64,
    },
    Stderr {
        data: String,
        #[serde(rename = "nextOffset")]
        next: u64,
    },
    Heartbeat,
    Exit {
        status: String,
        #[serde(rename = "exitCode")]
        code: Option<i32>,
        signal: Option<String>,
        #[serde(rename = "stdoutOffset")]
        stdout: u64,
        #[serde(rename = "stderrOffset")]
        stderr: u64,
    },
    Reconnect {
        #[serde(rename = "stdoutOffset")]
        stdout: u64,
        #[serde(rename = "stderrOffset")]
        stderr: u64,
    },
    Error,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Hello,
    Started,
    Stdout,
    Stderr,
    Exited,
    Success,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inner {
    version: u8,
    attempt: String,
    kind: Kind,
    pid: Option<u32>,
    data: Option<String>,
    exit_code: Option<i32>,
    exited: Option<bool>,
}
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Hello,
    Start,
    Running,
    Ended,
    Success,
}

pub(super) struct Decoder {
    pub(super) stdout: u64,
    pub(super) stderr: u64,
    pub(super) process: u64,
    pub(super) attempt: String,
    inner: Lines,
    phase: Phase,
    pub(super) attached: bool,
    pub(super) terminal: Option<bool>,
}

pub(super) struct Batch {
    pub(super) events: Vec<ProcessStreamEvent>,
    pub(super) success: bool,
    pub(super) failure: bool,
}

impl Decoder {
    pub(super) fn new(process: u64, attempt: String) -> Self {
        Self {
            stdout: 0,
            stderr: 0,
            process,
            attempt,
            inner: Lines::default(),
            phase: Phase::Hello,
            attached: false,
            terminal: None,
        }
    }
    pub(super) fn reconnect(&mut self) -> Result<()> {
        if self.terminal != Some(false) {
            return Err(Error::InvalidResponse);
        }
        self.attached = false;
        self.terminal = None;
        Ok(())
    }
    pub(super) fn outer(&mut self, line: &[u8]) -> Result<Batch> {
        if self.terminal.is_some() {
            return Err(Error::InvalidResponse);
        }
        let mut batch = Batch {
            events: Vec::new(),
            success: false,
            failure: false,
        };
        match decode::<Outer>(line)? {
            Outer::Attached {
                process,
                stdout,
                stderr,
            } if !self.attached
                && process == self.process
                && stdout == self.stdout
                && stderr == self.stderr =>
            {
                self.attached = true;
            }
            Outer::Stdout { data, next } if self.attached => {
                if data.is_empty() || self.stdout.checked_add(data.len() as u64) != Some(next) {
                    return Err(Error::InvalidResponse);
                }
                self.stdout = next;
                let mut bytes = data.as_bytes();
                while !bytes.is_empty() {
                    let line = match self.inner.take(&mut bytes, 64 * 1024) {
                        Ok(Some(line)) => line,
                        Ok(None) => break,
                        Err(_) => {
                            batch.failure = true;
                            break;
                        }
                    };
                    match self.event(&line) {
                        Ok(Some(event)) => batch.events.push(event),
                        Ok(None) => {
                            if self.phase == Phase::Success {
                                batch.success = true;
                            }
                        }
                        Err(_) => {
                            batch.failure = true;
                            break;
                        }
                    }
                }
            }
            Outer::Stderr { data, next }
                if self.attached && data.is_empty() && next == self.stderr => {}
            Outer::Heartbeat if self.attached => {}
            Outer::Reconnect { stdout, stderr }
                if self.attached && stdout == self.stdout && stderr == self.stderr =>
            {
                self.terminal = Some(false);
            }
            Outer::Exit {
                status,
                code: Some(0),
                signal: None,
                stdout,
                stderr,
            } if self.attached
                && self.phase == Phase::Success
                && self.inner.empty()
                && status == "exited"
                && stdout == self.stdout
                && stderr == self.stderr =>
            {
                self.terminal = Some(true);
            }
            _ => return Err(Error::InvalidResponse),
        }
        Ok(batch)
    }
    fn event(&mut self, line: &[u8]) -> Result<Option<ProcessStreamEvent>> {
        let value: Inner = decode(line)?;
        if value.version != 1 || value.attempt != self.attempt {
            return Err(Error::InvalidResponse);
        }
        let event = match value.kind {
            Kind::Hello
                if self.phase == Phase::Hello
                    && value.pid.is_none()
                    && value.data.is_none()
                    && value.exit_code.is_none()
                    && value.exited.is_none() =>
            {
                self.phase = Phase::Start;
                None
            }
            Kind::Started
                if self.phase == Phase::Start
                    && value.pid.is_some_and(|pid| pid > 0)
                    && value.data.is_none()
                    && value.exit_code.is_none()
                    && value.exited.is_none() =>
            {
                self.phase = Phase::Running;
                value.pid.map(|pid| ProcessStreamEvent::Started { pid })
            }
            Kind::Stdout | Kind::Stderr
                if self.phase == Phase::Running
                    && value.pid.is_none()
                    && value.exit_code.is_none()
                    && value.exited.is_none() =>
            {
                let Some(data) = value.data else {
                    return Err(Error::InvalidResponse);
                };
                let data = match STANDARD.decode(data) {
                    Ok(data) if data.len() <= 16384 => data,
                    _ => return Err(Error::InvalidResponse),
                };
                Some(if matches!(value.kind, Kind::Stdout) {
                    ProcessStreamEvent::Stdout(data)
                } else {
                    ProcessStreamEvent::Stderr(data)
                })
            }
            Kind::Exited
                if self.phase == Phase::Running && value.pid.is_none() && value.data.is_none() =>
            {
                let (Some(exit_code), Some(exited)) = (value.exit_code, value.exited) else {
                    return Err(Error::InvalidResponse);
                };
                self.phase = Phase::Ended;
                Some(ProcessStreamEvent::Exited { exit_code, exited })
            }
            Kind::Success
                if self.phase == Phase::Ended
                    && value.pid.is_none()
                    && value.data.is_none()
                    && value.exit_code.is_none()
                    && value.exited.is_none() =>
            {
                self.phase = Phase::Success;
                None
            }
            _ => return Err(Error::InvalidResponse),
        };
        Ok(event)
    }
}
