//! Streaming collection and bounded helper-command support.

use std::time::Duration;

use futures_util::StreamExt;
use serde::Serialize;

use crate::error::{Error, Result};

use super::connect::ConnectProcessTransport;
use super::framing::{FrameDecoder, ProcessEvent, decode_end_stream, decode_event};
use super::helper_run::KILL_DEADLINE;
use super::selector::ProcessSelector;
use super::types::{ProcessConnection, ProcessOutputCapture};
use super::wire::encode;

const MAX_FRAME_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum CollectionMode {
    StartPersistent,
    ObservePersistent,
    RunOneShot,
}

#[derive(Clone, Copy)]
pub(super) struct CollectionDeadlines {
    pub(super) execution: tokio::time::Instant,
    pub(super) cleanup: Option<tokio::time::Instant>,
}

impl ConnectProcessTransport {
    pub(super) async fn collect(
        &self,
        connection: ProcessConnection,
        method: &str,
        request: &impl Serialize,
        wait: Duration,
        output_capture: ProcessOutputCapture,
        mode: CollectionMode,
    ) -> Result<CollectedEvents> {
        let execution_deadline = tokio::time::Instant::now()
            .checked_add(wait)
            .ok_or(Error::InvalidRequest)?;
        self.collect_before(
            connection,
            method,
            request,
            CollectionDeadlines {
                execution: execution_deadline,
                cleanup: None,
            },
            output_capture,
            mode,
        )
        .await
    }

    pub(super) async fn collect_before(
        &self,
        connection: ProcessConnection,
        method: &str,
        request: &impl Serialize,
        deadlines: CollectionDeadlines,
        output_capture: ProcessOutputCapture,
        mode: CollectionMode,
    ) -> Result<CollectedEvents> {
        let request = encode(request)?;
        let mut stream = match tokio::time::timeout_at(
            deadlines.execution,
            self.http
                .stream(connection.clone(), method.to_owned(), request),
        )
        .await
        {
            Ok(stream) => stream?,
            Err(_) => return Ok(CollectedEvents::default()),
        };
        let mut decoder = FrameDecoder::new(MAX_FRAME_BYTES);
        let mut collected = CollectedEvents::default();
        let collection: Result<()> = async {
            let mut process_ended = false;
            let mut trailer_consumed = false;
            let mut reached_eof = false;
            while let Some(remaining) = deadlines
                .execution
                .checked_duration_since(tokio::time::Instant::now())
            {
                let fragment = match tokio::time::timeout(remaining, stream.next()).await {
                    Ok(Some(fragment)) => fragment?,
                    Ok(None) => {
                        reached_eof = true;
                        break;
                    }
                    Err(_) => break,
                };
                let decoded = decoder.push(&fragment);
                let mut complete = false;
                for frame in decoded.frames {
                    if frame.end_stream {
                        decode_end_stream(&frame.payload)?;
                        trailer_consumed = true;
                        break;
                    }
                    if process_ended {
                        return Err(Error::MalformedFrame);
                    }
                    match decode_event(&frame.payload)? {
                        ProcessEvent::Start(pid) => {
                            collected.pid = Some(pid);
                            if mode == CollectionMode::StartPersistent {
                                complete = true;
                                break;
                            }
                        }
                        ProcessEvent::Data { bytes, .. } => {
                            collected.capture(bytes, output_capture)?;
                        }
                        ProcessEvent::End { exit_code, exited } => {
                            collected.exit_code = Some(exit_code);
                            collected.exited = exited;
                            process_ended = true;
                        }
                        ProcessEvent::KeepAlive => {}
                    }
                }
                if let Some(error) = decoded.terminal_error {
                    return Err(error);
                }
                if complete {
                    return Ok(());
                }
            }
            if (process_ended || trailer_consumed) && !(trailer_consumed && reached_eof) {
                Err(Error::MalformedFrame)
            } else {
                Ok(())
            }
        }
        .await;
        if mode == CollectionMode::RunOneShot && collected.exit_code.is_none() {
            self.kill_best_effort_by(connection, collected.pid, deadlines.cleanup)
                .await;
        }
        collection?;
        Ok(collected)
    }

    pub(super) async fn kill_best_effort(&self, connection: ProcessConnection, pid: Option<u32>) {
        self.kill_best_effort_by(connection, pid, None).await;
    }

    async fn kill_best_effort_by(
        &self,
        connection: ProcessConnection,
        pid: Option<u32>,
        completion_deadline: Option<tokio::time::Instant>,
    ) {
        let Some(pid) = pid else {
            return;
        };
        let Ok(request) = encode(&super::wire::signal(ProcessSelector::Pid(pid))) else {
            return;
        };
        let kill = self
            .http
            .unary(connection, "SendSignal".to_owned(), request, false);
        let Some(relative_deadline) = tokio::time::Instant::now().checked_add(KILL_DEADLINE) else {
            return;
        };
        let deadline = completion_deadline.map_or(relative_deadline, |absolute| {
            absolute.min(relative_deadline)
        });
        if !matches!(tokio::time::timeout_at(deadline, kill).await, Ok(Ok(_))) {
            tracing::debug!(event = "e2b_process_kill_unconfirmed");
        }
    }
}

#[derive(Default)]
pub(super) struct CollectedEvents {
    pub(super) pid: Option<u32>,
    pub(super) bytes: Vec<u8>,
    pub(super) exit_code: Option<i32>,
    pub(super) exited: bool,
    pub(super) output_truncated: bool,
}

impl CollectedEvents {
    fn capture(&mut self, bytes: Vec<u8>, policy: ProcessOutputCapture) -> Result<()> {
        match policy {
            ProcessOutputCapture::HardLimit { max_bytes } => {
                if self.bytes.len().saturating_add(bytes.len()) > max_bytes {
                    return Err(Error::ResponseTooLarge);
                }
                self.bytes.extend_from_slice(&bytes);
            }
            ProcessOutputCapture::Tail { max_bytes } => {
                let overflow = self.bytes.len().saturating_add(bytes.len()) > max_bytes;
                if !overflow {
                    self.bytes.extend_from_slice(&bytes);
                    return Ok(());
                }
                self.output_truncated = true;
                if bytes.len() >= max_bytes {
                    self.bytes.clear();
                    self.bytes
                        .extend_from_slice(&bytes[bytes.len().saturating_sub(max_bytes)..]);
                } else {
                    let retained = max_bytes.saturating_sub(bytes.len());
                    let remove = self.bytes.len().saturating_sub(retained);
                    self.bytes.drain(..remove);
                    self.bytes.extend_from_slice(&bytes);
                }
            }
        }
        Ok(())
    }
}
