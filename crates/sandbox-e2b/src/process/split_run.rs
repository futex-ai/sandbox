//! Split-stream bounded non-interactive process collection.

use futures_util::StreamExt;

use crate::error::Result;

use super::connect::ConnectProcessTransport;
use super::framing::{FrameDecoder, ProcessDataChannel, ProcessEvent, decode_event};
use super::types::{ProcessConnection, ProcessSplitOutput, SplitProcessCommand};
use super::wire::{argv_start, encode, signal};

const MAX_FRAME_BYTES: usize = 1024 * 1024;
const KILL_DEADLINE: std::time::Duration = std::time::Duration::from_secs(3);

impl ConnectProcessTransport {
    pub(super) async fn collect_split(
        &self,
        connection: ProcessConnection,
        command: SplitProcessCommand,
    ) -> Result<ProcessSplitOutput> {
        let SplitProcessCommand {
            command,
            args,
            stdout_limit,
            stderr_limit,
            deadline,
        } = command;
        let absolute_deadline = tokio::time::Instant::now() + deadline;
        let limits = StreamLimits {
            stdout: stdout_limit,
            stderr: stderr_limit,
        };
        let request = encode(&argv_start(command, args))?;
        let stream = self
            .http
            .stream(connection.clone(), "Start".to_owned(), request);
        let mut stream = match tokio::time::timeout_at(absolute_deadline, stream).await {
            Ok(result) => result?,
            Err(_) => return Ok(ProcessSplitOutput::default()),
        };
        let mut decoder = FrameDecoder::new(MAX_FRAME_BYTES);
        let mut collected = ProcessSplitOutput::default();
        let mut pid = None;
        let collection: Result<()> = async {
            'stream: while let Some(remaining) =
                absolute_deadline.checked_duration_since(tokio::time::Instant::now())
            {
                let fragment = match tokio::time::timeout(remaining, stream.next()).await {
                    Ok(Some(fragment)) => fragment?,
                    Ok(None) | Err(_) => break,
                };
                let decoded = decoder.push(&fragment);
                let mut stop = false;
                for frame in decoded.frames {
                    if frame.end_stream {
                        stop = true;
                        break;
                    }
                    match decode_event(&frame.payload)? {
                        ProcessEvent::Start(started) => pid = Some(started),
                        ProcessEvent::Data { channel, bytes } => {
                            append(&mut collected, channel, &bytes, &limits);
                            if collected.stdout_overflowed || collected.stderr_overflowed {
                                stop = true;
                                break;
                            }
                        }
                        ProcessEvent::End { exit_code, exited } => {
                            collected.exit_code = Some(exit_code);
                            collected.exited = exited;
                            stop = true;
                            break;
                        }
                        ProcessEvent::KeepAlive => {}
                    }
                }
                if let Some(error) = decoded.terminal_error {
                    return Err(error);
                }
                if stop {
                    break 'stream;
                }
            }
            Ok(())
        }
        .await;
        if collected.exit_code.is_none() {
            self.kill_best_effort(connection, pid).await;
        }
        collection?;
        Ok(collected)
    }

    async fn kill_best_effort(&self, connection: ProcessConnection, pid: Option<u32>) {
        let Some(pid) = pid else {
            return;
        };
        let Ok(request) = encode(&signal(pid)) else {
            return;
        };
        let kill = self
            .http
            .unary(connection, "SendSignal".to_owned(), request, false);
        if !matches!(tokio::time::timeout(KILL_DEADLINE, kill).await, Ok(Ok(_))) {
            tracing::debug!(event = "e2b_split_run_kill_unconfirmed");
        }
    }
}

struct StreamLimits {
    stdout: usize,
    stderr: usize,
}

fn append(
    collected: &mut ProcessSplitOutput,
    channel: ProcessDataChannel,
    bytes: &[u8],
    limits: &StreamLimits,
) {
    match channel {
        ProcessDataChannel::Stdout | ProcessDataChannel::Pty => append_bounded(
            &mut collected.stdout,
            &mut collected.stdout_overflowed,
            bytes,
            limits.stdout,
        ),
        ProcessDataChannel::Stderr => append_bounded(
            &mut collected.stderr,
            &mut collected.stderr_overflowed,
            bytes,
            limits.stderr,
        ),
    }
}

fn append_bounded(target: &mut Vec<u8>, overflowed: &mut bool, bytes: &[u8], limit: usize) {
    let remaining = limit.saturating_sub(target.len());
    if bytes.len() > remaining {
        *overflowed = true;
    }
    target.extend_from_slice(&bytes[..bytes.len().min(remaining)]);
}

#[cfg(test)]
#[path = "_tests_/split_run_tests.rs"]
mod split_run_tests;
