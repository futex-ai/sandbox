//! Streaming collection and bounded helper-command support.

use std::time::Duration;

use futures_util::StreamExt;
use sandbox_interface::{Error as DomainError, Result as DomainResult};
use serde::Serialize;

use crate::error::{Error, Result};

use super::connect::ConnectProcessTransport;
use super::framing::{FrameDecoder, ProcessEvent, decode_end_stream, decode_event};
use super::mapping::map_result;
use super::selector::ProcessSelector;
use super::types::{ProcessCommand, ProcessConnection, ProcessOutputCapture, ProcessRunOutput};
use super::wire::{command_start, encode};

const MAX_FRAME_BYTES: usize = 1024 * 1024;
const KILL_DEADLINE: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum CollectionMode {
    StartPersistent,
    ObservePersistent,
    RunOneShot,
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
        let request = encode(request)?;
        let deadline = tokio::time::Instant::now()
            .checked_add(wait)
            .ok_or(Error::InvalidRequest)?;
        let mut stream = match tokio::time::timeout_at(
            deadline,
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
            while let Some(remaining) = deadline.checked_duration_since(tokio::time::Instant::now())
            {
                let fragment = match tokio::time::timeout(remaining, stream.next()).await {
                    Ok(Some(fragment)) => fragment?,
                    Ok(None) | Err(_) => break,
                };
                let decoded = decoder.push(&fragment);
                let mut complete = false;
                for frame in decoded.frames {
                    if frame.end_stream {
                        decode_end_stream(&frame.payload)?;
                        complete = true;
                        break;
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
                    return Ok(());
                }
            }
            Ok(())
        }
        .await;
        if mode == CollectionMode::RunOneShot && collected.exit_code.is_none() {
            self.kill_best_effort(connection, collected.pid).await;
        }
        collection?;
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
            .collect(
                connection,
                "Start",
                &body,
                timeout,
                output_capture,
                CollectionMode::RunOneShot,
            )
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
        match self.run_for(connection, command, timeout).await {
            Ok(output) if output.exited && output.exit_code.is_some() => Ok(output),
            Err(error) => Err(error),
            Ok(_) => Err(DomainError::BackendUnavailable {
                backend_id: self.backend_id.clone(),
            }),
        }
    }

    pub(super) async fn kill_best_effort(&self, connection: ProcessConnection, pid: Option<u32>) {
        let Some(pid) = pid else {
            return;
        };
        let Ok(request) = encode(&super::wire::signal(ProcessSelector::Pid(pid))) else {
            return;
        };
        let kill = self
            .http
            .unary(connection, "SendSignal".to_owned(), request, false);
        if !matches!(tokio::time::timeout(KILL_DEADLINE, kill).await, Ok(Ok(_))) {
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
