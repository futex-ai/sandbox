//! Split-stream bounded non-interactive process collection.

use futures_util::StreamExt;

use crate::error::{Error, Result};

use super::connect::ConnectProcessTransport;
use super::framing::{
    FrameDecoder, ProcessDataChannel, ProcessEvent, decode_end_stream, decode_event,
};
use super::types::{ProcessConnection, ProcessSplitOutput, SplitProcessCommand};
use super::wire::{argv_start, encode};

const MAX_FRAME_BYTES: usize = 1024 * 1024;

impl ConnectProcessTransport {
    pub(super) async fn collect_split(
        &self,
        connection: ProcessConnection,
        command: SplitProcessCommand,
    ) -> Result<ProcessSplitOutput> {
        let SplitProcessCommand {
            command,
            args,
            cwd,
            envs,
            stdout_limit,
            stderr_limit,
            deadline,
        } = command;
        let absolute_deadline = tokio::time::Instant::now()
            .checked_add(deadline)
            .ok_or(Error::InvalidRequest)?;
        let limits = StreamLimits {
            stdout: stdout_limit,
            stderr: stderr_limit,
        };
        let request = encode(&argv_start(command, args, cwd, envs))?;
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
            let mut process_ended = false;
            let mut trailer_consumed = false;
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
                        decode_end_stream(&frame.payload)?;
                        trailer_consumed = true;
                        stop = true;
                        break;
                    }
                    if process_ended {
                        return Err(Error::MalformedFrame);
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
                            process_ended = true;
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
            if process_ended && !trailer_consumed {
                Err(Error::MalformedFrame)
            } else {
                Ok(())
            }
        }
        .await;
        if collected.exit_code.is_none() {
            self.kill_best_effort(connection, pid).await;
        }
        collection?;
        Ok(collected)
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
