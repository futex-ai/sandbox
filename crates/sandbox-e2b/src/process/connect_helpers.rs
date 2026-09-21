//! Streaming collection and bounded helper-command support.

use std::time::Duration;

use futures_util::StreamExt;
use sandbox_interface::{Error as DomainError, Result as DomainResult};
use serde::Serialize;

use crate::error::{Error, Result};

use super::connect::ConnectProcessTransport;
use super::framing::{FrameDecoder, ProcessEvent, decode_event};
use super::mapping::map_result;
use super::types::{ProcessCommand, ProcessConnection, ProcessOutputCapture, ProcessRunOutput};
use super::wire::{command_start, encode};

const MAX_FRAME_BYTES: usize = 1024 * 1024;

impl ConnectProcessTransport {
    pub(super) async fn collect(
        &self,
        connection: ProcessConnection,
        method: &str,
        request: &impl Serialize,
        wait: Duration,
        output_capture: ProcessOutputCapture,
        return_after_start: bool,
    ) -> Result<CollectedEvents> {
        let request = encode(request)?;
        let deadline = tokio::time::Instant::now() + wait;
        let mut stream = match tokio::time::timeout_at(
            deadline,
            self.http.stream(connection, method.to_owned(), request),
        )
        .await
        {
            Ok(stream) => stream?,
            Err(_) => return Ok(CollectedEvents::default()),
        };
        let mut decoder = FrameDecoder::new(MAX_FRAME_BYTES);
        let mut collected = CollectedEvents::default();
        while let Some(remaining) = deadline.checked_duration_since(tokio::time::Instant::now()) {
            let fragment = match tokio::time::timeout(remaining, stream.next()).await {
                Ok(Some(fragment)) => fragment?,
                Ok(None) | Err(_) => break,
            };
            let decoded = decoder.push(&fragment);
            let mut complete = false;
            for frame in decoded.frames {
                if frame.end_stream {
                    complete = true;
                    break;
                }
                match decode_event(&frame.payload)? {
                    ProcessEvent::Start(pid) => {
                        collected.pid = Some(pid);
                        if return_after_start {
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
                        complete = true;
                        break;
                    }
                    ProcessEvent::KeepAlive => {}
                }
            }
            if let Some(error) = decoded.terminal_error {
                return Err(error);
            }
            if complete {
                return Ok(collected);
            }
        }
        Ok(collected)
    }

    pub(super) async fn run_for(
        &self,
        connection: ProcessConnection,
        command: ProcessCommand,
        timeout: Duration,
    ) -> DomainResult<ProcessRunOutput> {
        let output_capture = command.output_capture;
        let read_only = command.read_only;
        let body = command_start(command);
        let collected = self
            .collect(connection, "Start", &body, timeout, output_capture, false)
            .await;
        let events = match collected {
            Err(Error::ResponseTooLarge) if read_only => {
                return Err(DomainError::ReadOnlyOutputTooLarge);
            }
            result => map_result(result, false, &self.backend_id)?,
        };
        Ok(ProcessRunOutput {
            bytes: events.bytes,
            exit_code: events.exit_code,
            exited: events.exited,
            output_truncated: events.output_truncated,
        })
    }

    pub(super) async fn run_helper(
        &self,
        connection: ProcessConnection,
        command: ProcessCommand,
        timeout: Duration,
    ) -> DomainResult<ProcessRunOutput> {
        match tokio::time::timeout(timeout, self.run_for(connection, command, timeout)).await {
            Ok(Ok(output)) if output.exit_code.is_some() => Ok(output),
            Ok(Err(error)) => Err(error),
            Ok(Ok(_)) | Err(_) => Err(DomainError::BackendUnavailable {
                backend_id: self.backend_id.clone(),
            }),
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
