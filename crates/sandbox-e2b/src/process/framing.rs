//! Incremental Connect streaming envelope and process-event decoding.

use std::mem;

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;

use crate::error::{Error, Result};

use super::provider_pid::ProviderPid;

const HEADER_BYTES: usize = 5;

pub(super) struct FrameDecoder {
    buffer: Vec<u8>,
    maximum_payload: usize,
    /// A terminal envelope makes every subsequently supplied byte malformed.
    end_stream_seen: bool,
}

impl FrameDecoder {
    pub(super) fn new(maximum_payload: usize) -> Self {
        Self {
            buffer: Vec::new(),
            maximum_payload,
            end_stream_seen: false,
        }
    }

    pub(super) fn push(&mut self, mut fragment: &[u8]) -> DecodedFrameBatch {
        if self.end_stream_seen && !fragment.is_empty() {
            return DecodedFrameBatch::terminal(Vec::new(), Error::MalformedFrame);
        }
        let mut frames = Vec::new();
        while !fragment.is_empty() {
            if self.buffer.len() < HEADER_BYTES {
                let copied = (HEADER_BYTES - self.buffer.len()).min(fragment.len());
                self.buffer.extend_from_slice(&fragment[..copied]);
                fragment = &fragment[copied..];
                if self.buffer.len() < HEADER_BYTES {
                    break;
                }
            }
            let flags = self.buffer[0];
            if flags & !0x02 != 0 {
                self.buffer.clear();
                return DecodedFrameBatch::terminal(frames, Error::MalformedFrame);
            }
            let length = u32::from_be_bytes([
                self.buffer[1],
                self.buffer[2],
                self.buffer[3],
                self.buffer[4],
            ]) as usize;
            if length > self.maximum_payload {
                self.buffer.clear();
                return DecodedFrameBatch::terminal(frames, Error::ResponseTooLarge);
            }
            let frame_length = HEADER_BYTES.saturating_add(length);
            let copied = (frame_length - self.buffer.len()).min(fragment.len());
            self.buffer.extend_from_slice(&fragment[..copied]);
            fragment = &fragment[copied..];
            if self.buffer.len() < frame_length {
                break;
            }
            let payload = mem::take(&mut self.buffer).split_off(HEADER_BYTES);
            let end_stream = flags & 0x02 != 0;
            frames.push(ConnectFrame {
                end_stream,
                payload,
            });
            if end_stream {
                self.end_stream_seen = true;
                if !fragment.is_empty() {
                    return DecodedFrameBatch::terminal(frames, Error::MalformedFrame);
                }
                break;
            }
        }
        DecodedFrameBatch {
            frames,
            terminal_error: None,
        }
    }
}

/// Complete frames decoded before an optional terminal framing error.
pub(super) struct DecodedFrameBatch {
    pub(super) frames: Vec<ConnectFrame>,
    pub(super) terminal_error: Option<Error>,
}

impl DecodedFrameBatch {
    fn terminal(frames: Vec<ConnectFrame>, error: Error) -> Self {
        Self {
            frames,
            terminal_error: Some(error),
        }
    }
}

pub(super) struct ConnectFrame {
    pub(super) end_stream: bool,
    pub(super) payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProcessDataChannel {
    Pty,
    Stdout,
    Stderr,
}

#[derive(Eq, PartialEq)]
pub(super) enum ProcessEvent {
    Start(u32),
    Data {
        channel: ProcessDataChannel,
        bytes: Vec<u8>,
    },
    End {
        exit_code: i32,
        exited: bool,
    },
    KeepAlive,
}

pub(super) fn decode_event(payload: &[u8]) -> Result<ProcessEvent> {
    let response: ResponseWire = match serde_json::from_slice(payload) {
        Ok(response) => response,
        Err(_) => return Err(Error::MalformedFrame),
    };
    let event = response.event;
    let variant_count = usize::from(event.start.is_some())
        + usize::from(event.data.is_some())
        + usize::from(event.end.is_some())
        + usize::from(event.keepalive.is_some());
    if variant_count != 1 {
        return Err(Error::MalformedFrame);
    }
    if let Some(start) = event.start {
        return Ok(ProcessEvent::Start(start.pid.get()));
    }
    if let Some(data) = event.data {
        let channel_count = usize::from(data.pty.is_some())
            + usize::from(data.stdout.is_some())
            + usize::from(data.stderr.is_some());
        if channel_count != 1 {
            return Err(Error::MalformedFrame);
        }
        let (channel, encoded) = if let Some(encoded) = data.pty {
            (ProcessDataChannel::Pty, encoded)
        } else if let Some(encoded) = data.stdout {
            (ProcessDataChannel::Stdout, encoded)
        } else if let Some(encoded) = data.stderr {
            (ProcessDataChannel::Stderr, encoded)
        } else {
            return Err(Error::MalformedFrame);
        };
        let bytes = match STANDARD.decode(encoded) {
            Ok(bytes) => bytes,
            Err(_) => return Err(Error::MalformedFrame),
        };
        return Ok(ProcessEvent::Data { channel, bytes });
    }
    if let Some(end) = event.end {
        return Ok(ProcessEvent::End {
            exit_code: end.exit_code,
            exited: end.exited,
        });
    }
    if event.keepalive.is_some() {
        return Ok(ProcessEvent::KeepAlive);
    }
    Err(Error::MalformedFrame)
}

pub(super) fn decode_end_stream(payload: &[u8]) -> Result<()> {
    let response: EndStreamWire = match serde_json::from_slice(payload) {
        Ok(response) => response,
        Err(_) => return Err(Error::MalformedFrame),
    };
    if response.error.is_some() {
        return Err(Error::Unavailable);
    }
    Ok(())
}

pub(super) fn encode_frame(payload: &[u8]) -> Result<Vec<u8>> {
    let length = match u32::try_from(payload.len()) {
        Ok(length) => length,
        Err(_) => return Err(Error::ResponseTooLarge),
    };
    let mut frame = Vec::with_capacity(HEADER_BYTES + payload.len());
    frame.push(0);
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

#[derive(Deserialize)]
struct ResponseWire {
    event: EventWire,
}

#[derive(Deserialize)]
struct EventWire {
    start: Option<StartWire>,
    data: Option<DataWire>,
    end: Option<EndWire>,
    keepalive: Option<KeepAliveWire>,
}

#[derive(Deserialize)]
struct StartWire {
    pid: ProviderPid,
}

#[derive(Deserialize)]
struct DataWire {
    pty: Option<String>,
    stdout: Option<String>,
    stderr: Option<String>,
}

#[derive(Deserialize)]
struct EndWire {
    #[serde(default, rename = "exitCode")]
    exit_code: i32,
    #[serde(default)]
    exited: bool,
}

#[derive(Deserialize)]
struct KeepAliveWire {}

#[derive(Deserialize)]
struct EndStreamWire {
    #[serde(default)]
    error: Option<EndStreamErrorWire>,
}

#[derive(Deserialize)]
struct EndStreamErrorWire {}

#[cfg(test)]
#[path = "_tests_/framing_tests.rs"]
mod framing_tests;
