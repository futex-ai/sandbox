//! Incremental split-stream process execution.

use std::time::Duration;

use futures_util::{StreamExt, stream};
use sandbox_interface::{
    Error as DomainError, ProcessEventStream, ProcessStreamEvent, ProcessStreamOutcome,
    Result as DomainResult,
};
use tokio::sync::mpsc::{self, Sender};

use super::{
    connect::ConnectProcessTransport,
    framing::{FrameDecoder, ProcessDataChannel, ProcessEvent, decode_end_stream, decode_event},
    mapping::map_result,
    stream_state::{
        Completion, Delivery, EventResult, StreamSettings, StreamState, deliver, finish_stream,
    },
    types::{ProcessConnection, StreamProcessCommand},
    wire::{argv_start, encode},
};

const EVENT_CHANNEL_CAPACITY: usize = 16;
const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub(super) const STREAM_TRANSPORT_ALLOWANCE: Duration = Duration::from_secs(10);

impl ConnectProcessTransport {
    pub(super) fn stream_events(
        &self,
        connection: ProcessConnection,
        command: StreamProcessCommand,
    ) -> DomainResult<ProcessEventStream> {
        let StreamProcessCommand {
            command,
            args,
            stdout_limit,
            stderr_limit,
            deadline,
            idle_timeout,
        } = command;
        let request = map_result(encode(&argv_start(command, args)), false, &self.backend_id)?;
        let started_at = tokio::time::Instant::now();
        let absolute_deadline = started_at
            .checked_add(deadline)
            .ok_or_else(|| DomainError::internal_message("stream process deadline overflow"))?;
        let idle_deadline = started_at
            .checked_add(idle_timeout)
            .ok_or_else(|| DomainError::internal_message("stream process idle timeout overflow"))?;
        let settings = StreamSettings {
            request,
            stdout_limit,
            stderr_limit,
            absolute_deadline,
            idle_timeout,
            request_timeout: deadline.saturating_add(STREAM_TRANSPORT_ALLOWANCE),
        };
        let (sender, receiver) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let transport = self.clone();
        let _worker = tokio::spawn(async move {
            transport
                .produce_events(connection, settings, sender, idle_deadline)
                .await;
        });
        let events = stream::unfold(receiver, |mut receiver| async move {
            receiver.recv().await.map(|event| (event, receiver))
        });
        Ok(Box::pin(events))
    }

    async fn produce_events(
        &self,
        connection: ProcessConnection,
        settings: StreamSettings,
        sender: Sender<ProcessStreamEvent>,
        idle_deadline: tokio::time::Instant,
    ) {
        let mut state = StreamState::new(idle_deadline);
        let completion = self
            .drive_stream(connection.clone(), &settings, &sender, &mut state)
            .await;
        finish_stream(sender, completion).await;
        if !state.ended {
            self.kill_best_effort(connection, state.pid).await;
        }
    }

    async fn drive_stream(
        &self,
        connection: ProcessConnection,
        settings: &StreamSettings,
        sender: &Sender<ProcessStreamEvent>,
        state: &mut StreamState,
    ) -> Completion {
        let opening = self.http.stream_with_timeout(
            connection,
            "Start".to_owned(),
            settings.request.clone(),
            settings.request_timeout,
        );
        let mut provider_stream = tokio::select! {
            biased;
            _ = sender.closed() => return Completion::ConsumerDropped,
            _ = tokio::time::sleep_until(settings.absolute_deadline) => {
                return Completion::Outcome(ProcessStreamOutcome::DeadlineExpired);
            }
            _ = tokio::time::sleep_until(state.idle_deadline) => {
                return Completion::Outcome(ProcessStreamOutcome::IdleTimeout);
            }
            result = opening => match result {
                Ok(provider_stream) => provider_stream,
                Err(_) => return Completion::Outcome(ProcessStreamOutcome::TransportFailure),
            },
        };
        let mut decoder = FrameDecoder::new(MAX_FRAME_BYTES);
        loop {
            let fragment = tokio::select! {
                biased;
                _ = sender.closed() => return Completion::ConsumerDropped,
                _ = tokio::time::sleep_until(settings.absolute_deadline) => {
                    return Completion::Outcome(ProcessStreamOutcome::DeadlineExpired);
                }
                _ = tokio::time::sleep_until(state.idle_deadline) => {
                    return Completion::Outcome(ProcessStreamOutcome::IdleTimeout);
                }
                item = provider_stream.next() => match item {
                    Some(Ok(fragment)) => fragment,
                    Some(Err(_)) | None => {
                        return Completion::Outcome(ProcessStreamOutcome::TransportFailure);
                    }
                },
            };
            let decoded = decoder.push(&fragment);
            let mut batch_outcome = None;
            for frame in decoded.frames {
                if frame.end_stream {
                    batch_outcome = Some(
                        if state.ended && decode_end_stream(&frame.payload).is_ok() {
                            ProcessStreamOutcome::Completed
                        } else {
                            ProcessStreamOutcome::TransportFailure
                        },
                    );
                    break;
                }
                if state.ended {
                    batch_outcome = Some(ProcessStreamOutcome::TransportFailure);
                    break;
                }
                let event = match decode_event(&frame.payload) {
                    Ok(event) => event,
                    Err(_) => {
                        batch_outcome = Some(ProcessStreamOutcome::TransportFailure);
                        break;
                    }
                };
                match self.handle_event(event, settings, sender, state).await {
                    EventResult::Continue => {}
                    EventResult::Complete(completion) => return completion,
                    EventResult::Outcome(outcome) => {
                        batch_outcome = Some(outcome);
                        break;
                    }
                }
            }
            if decoded.terminal_error.is_some() {
                return Completion::Outcome(ProcessStreamOutcome::TransportFailure);
            }
            if let Some(outcome) = batch_outcome {
                return Completion::Outcome(outcome);
            }
        }
    }

    async fn handle_event(
        &self,
        event: ProcessEvent,
        settings: &StreamSettings,
        sender: &Sender<ProcessStreamEvent>,
        state: &mut StreamState,
    ) -> EventResult {
        let (event, overflow) = match event {
            ProcessEvent::Start(pid) if !state.started => {
                state.started = true;
                state.pid = Some(pid);
                (Some(ProcessStreamEvent::Started { pid }), None)
            }
            ProcessEvent::Start(_) => return EventResult::transport_failure(),
            ProcessEvent::Data { channel, bytes } if state.started => {
                if !bytes.is_empty()
                    && matches!(
                        channel,
                        ProcessDataChannel::Stdout | ProcessDataChannel::Stderr
                    )
                {
                    let Some(deadline) =
                        tokio::time::Instant::now().checked_add(settings.idle_timeout)
                    else {
                        return EventResult::transport_failure();
                    };
                    state.idle_deadline = deadline;
                }
                state.capture(channel, bytes, settings)
            }
            ProcessEvent::Data { .. } => return EventResult::transport_failure(),
            ProcessEvent::End { exit_code, exited } if state.started => {
                state.ended = true;
                (Some(ProcessStreamEvent::Exited { exit_code, exited }), None)
            }
            ProcessEvent::End { .. } => return EventResult::transport_failure(),
            ProcessEvent::KeepAlive => (None, None),
        };
        if let Some(event) = event {
            match deliver(event, settings, sender, state.idle_deadline).await {
                Delivery::Sent => {}
                Delivery::ConsumerDropped => {
                    return EventResult::Complete(Completion::ConsumerDropped);
                }
                Delivery::Outcome(outcome) => return EventResult::Outcome(outcome),
            }
        }
        match overflow {
            Some(outcome) => EventResult::Outcome(outcome),
            None => EventResult::Continue,
        }
    }
}

#[cfg(test)]
#[path = "_tests_/stream_run/mod.rs"]
mod stream_run_tests;
