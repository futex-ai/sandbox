//! Incremental split-stream process execution.

use std::{
    collections::BTreeMap,
    future::{Future, poll_fn},
    time::Duration,
};

use futures_util::stream;
use sandbox_interface::{
    Error as DomainError, ProcessEventStream, ProcessStreamEvent, ProcessStreamOutcome,
    Result as DomainResult,
};
use tokio::sync::{
    mpsc::{self, Sender},
    oneshot, watch,
};

use super::{
    connect::ConnectProcessTransport,
    framing::ProcessEvent,
    mapping::map_result,
    stream_drop::{await_pid_on_drop, drop_grace_deadline},
    stream_reader::{BufferedReader, ReaderObservation, ReaderTiming, StagedEvent},
    stream_state::{
        Completion, Delivery, EventReceiver, EventResult, StreamSettings, StreamState,
        StreamTerminal, deliver, finish_stream,
    },
    types::{ProcessConnection, StreamProcessCommand},
    wire::{argv_start, encode},
};

const EVENT_CHANNEL_CAPACITY: usize = 16;
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
            requested_at,
            deadline,
            idle_timeout,
        } = command;
        let request = map_result(
            encode(&argv_start(command, args, None, BTreeMap::new())),
            false,
            &self.backend_id,
        )?;
        let started_at = tokio::time::Instant::now();
        let absolute_deadline = requested_at
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
            request_timeout: absolute_deadline
                .saturating_duration_since(started_at)
                .saturating_add(STREAM_TRANSPORT_ALLOWANCE),
        };
        let (sender, receiver) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let (outcome_sender, outcome_receiver) = oneshot::channel();
        let transport = self.clone();
        let _worker = tokio::spawn(async move {
            transport
                .produce_events(connection, settings, sender, outcome_sender, idle_deadline)
                .await;
        });
        let events = stream::unfold(EventReceiver::new(receiver, outcome_receiver), |receiver| {
            receiver.next_event()
        });
        Ok(Box::pin(events))
    }

    async fn produce_events(
        &self,
        connection: ProcessConnection,
        settings: StreamSettings,
        sender: Sender<ProcessStreamEvent>,
        outcome_sender: oneshot::Sender<StreamTerminal>,
        idle_deadline: tokio::time::Instant,
    ) {
        let mut state = StreamState::new(idle_deadline);
        let (observed_sender, observed_receiver) = watch::channel(ReaderObservation::default());
        let completion = self
            .drive_stream(
                connection.clone(),
                &settings,
                &sender,
                &mut state,
                observed_sender,
                observed_receiver.clone(),
            )
            .await;
        let observation = *observed_receiver.borrow();
        state.pid = state.pid.or(observation.pid);
        state.ended |= observation.ended;
        finish_stream(
            sender,
            outcome_sender,
            completion,
            state.final_output.take(),
        );
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
        observed_sender: watch::Sender<ReaderObservation>,
        mut observed_receiver: watch::Receiver<ReaderObservation>,
    ) -> Completion {
        let opening = self.http.stream_with_timeout(
            connection,
            "Start".to_owned(),
            settings.request.clone(),
            settings.request_timeout,
        );
        tokio::pin!(opening);
        let mut open_polled = false;
        let mut drop_deadline = None;
        let provider_stream = tokio::select! {
            biased;
            _ = sender.closed() => {
                if !open_polled {
                    return Completion::ConsumerDropped;
                }
                let deadline = drop_grace_deadline(settings.absolute_deadline);
                drop_deadline = Some(deadline);
                match tokio::time::timeout_at(deadline, opening.as_mut()).await {
                    Ok(Ok(provider_stream)) => provider_stream,
                    _ => {
                        tracing::debug!(event = "e2b_stream_drop_pid_unobserved");
                        return Completion::ConsumerDropped;
                    }
                }
            },
            _ = tokio::time::sleep_until(settings.absolute_deadline) => {
                return Completion::Outcome(state.deadline_outcome());
            }
            _ = tokio::time::sleep_until(state.idle_deadline) => {
                return Completion::Outcome(state.idle_outcome());
            }
            result = poll_fn(|context| {
                open_polled = true;
                opening.as_mut().poll(context)
            }) => match result {
                Ok(provider_stream) => provider_stream,
                Err(_) => return Completion::Outcome(ProcessStreamOutcome::TransportFailure),
            },
        };
        let mut reader = BufferedReader::new(
            provider_stream,
            ReaderTiming {
                absolute_deadline: settings.absolute_deadline,
                idle_deadline: state.idle_deadline,
                idle_timeout: settings.idle_timeout,
                discovery_deadline: drop_deadline,
            },
            observed_sender,
            sender.clone(),
        );
        let mut trailer_received = false;
        let completion = async {
            loop {
                let event = tokio::select! {
                    biased;
                    _ = sender.closed() => {
                        let deadline = drop_deadline
                            .unwrap_or_else(|| drop_grace_deadline(settings.absolute_deadline));
                        await_pid_on_drop(&mut reader, &mut observed_receiver, deadline).await;
                        return Completion::ConsumerDropped;
                    },
                    _ = tokio::time::sleep_until(settings.absolute_deadline) => {
                        return Completion::Outcome(state.deadline_outcome());
                    }
                    _ = reader.outcome.changed(), if state.started => {
                        let result = (*reader.outcome.borrow())
                            .unwrap_or(ProcessStreamOutcome::TransportFailure);
                        return Completion::Outcome(result);
                    }
                    item = reader.events.recv() => match item {
                        None if trailer_received => {
                            return Completion::Outcome(ProcessStreamOutcome::Completed);
                        }
                        None => {
                            let result = (*reader.outcome.borrow())
                                .unwrap_or(ProcessStreamOutcome::TransportFailure);
                            return Completion::Outcome(result);
                        }
                        Some(event) => event,
                    },
                };
                match event {
                    StagedEvent::Trailer if state.ended && !trailer_received => {
                        trailer_received = true;
                    }
                    StagedEvent::Process(event) if !trailer_received && !state.ended => {
                        match self
                            .handle_event(event, settings, sender, state, &mut reader.outcome)
                            .await
                        {
                            EventResult::Continue => {}
                            EventResult::Complete(completion) => return completion,
                            EventResult::Outcome(outcome) => return Completion::Outcome(outcome),
                        }
                    }
                    StagedEvent::Failure | StagedEvent::Trailer | StagedEvent::Process(_) => {
                        return Completion::Outcome(ProcessStreamOutcome::TransportFailure);
                    }
                }
            }
        }
        .await;
        reader.stop().await;
        completion
    }

    async fn handle_event(
        &self,
        event: ProcessEvent,
        settings: &StreamSettings,
        sender: &Sender<ProcessStreamEvent>,
        state: &mut StreamState,
        outcome: &mut tokio::sync::watch::Receiver<Option<ProcessStreamOutcome>>,
    ) -> EventResult {
        let (event, overflow) = match event {
            ProcessEvent::Start(pid) if !state.started => {
                state.started = true;
                state.pid = Some(pid);
                (Some(ProcessStreamEvent::Started { pid }), None)
            }
            ProcessEvent::Start(_) => return EventResult::transport_failure(),
            ProcessEvent::Data { channel, bytes } if state.started => {
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
        if let Some(outcome) = overflow {
            state.final_output = event;
            return EventResult::Outcome(outcome);
        }
        if let Some(event) = event {
            match deliver(event, settings, sender, state, outcome).await {
                Delivery::Sent => {}
                Delivery::ConsumerDropped => {
                    return EventResult::Complete(Completion::ConsumerDropped);
                }
                Delivery::Outcome(outcome) => return EventResult::Outcome(outcome),
            }
        }
        EventResult::Continue
    }
}

#[cfg(test)]
#[path = "_tests_/stream_run/mod.rs"]
mod stream_run_tests;
