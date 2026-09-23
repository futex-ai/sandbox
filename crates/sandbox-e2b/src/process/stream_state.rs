//! State and bounded delivery helpers for process event streaming.

use std::time::Duration;

use sandbox_interface::{ProcessStreamEvent, ProcessStreamOutcome};
use tokio::sync::{
    mpsc::{Receiver, Sender},
    oneshot, watch,
};

use super::framing::ProcessDataChannel;

pub(super) struct StreamSettings {
    pub(super) request: Vec<u8>,
    pub(super) stdout_limit: usize,
    pub(super) stderr_limit: usize,
    pub(super) absolute_deadline: tokio::time::Instant,
    pub(super) idle_timeout: Duration,
    pub(super) request_timeout: Duration,
}

pub(super) struct StreamState {
    pub(super) pid: Option<u32>,
    pub(super) started: bool,
    pub(super) ended: bool,
    /// The bounded overflow prefix bypasses the full data queue with its outcome.
    pub(super) final_output: Option<ProcessStreamEvent>,
    stdout_bytes: usize,
    stderr_bytes: usize,
    pub(super) idle_deadline: tokio::time::Instant,
}

/// Drains ordered data events before yielding one separately published outcome.
pub(super) struct EventReceiver {
    events: Receiver<ProcessStreamEvent>,
    terminal: Option<oneshot::Receiver<StreamTerminal>>,
    outcome: Option<ProcessStreamOutcome>,
}

/// At most one bounded output prefix followed by exactly one terminal outcome.
pub(super) struct StreamTerminal {
    final_output: Option<ProcessStreamEvent>,
    outcome: ProcessStreamOutcome,
}

impl EventReceiver {
    pub(super) fn new(
        events: Receiver<ProcessStreamEvent>,
        terminal: oneshot::Receiver<StreamTerminal>,
    ) -> Self {
        Self {
            events,
            terminal: Some(terminal),
            outcome: None,
        }
    }

    /// Returns the next queued event or the terminal outcome after data closes.
    pub(super) async fn next_event(mut self) -> Option<(ProcessStreamEvent, EventReceiver)> {
        if let Some(event) = self.events.recv().await {
            return Some((event, self));
        }
        if let Some(outcome) = self.outcome.take() {
            return Some((ProcessStreamEvent::Outcome(outcome), self));
        }
        let receiver = self.terminal.take()?;
        let terminal = match receiver.await {
            Ok(terminal) => terminal,
            Err(_) => StreamTerminal {
                final_output: None,
                outcome: ProcessStreamOutcome::TransportFailure,
            },
        };
        if let Some(event) = terminal.final_output {
            self.outcome = Some(terminal.outcome);
            return Some((event, self));
        }
        Some((ProcessStreamEvent::Outcome(terminal.outcome), self))
    }
}

impl StreamState {
    pub(super) fn new(idle_deadline: tokio::time::Instant) -> Self {
        Self {
            pid: None,
            started: false,
            ended: false,
            final_output: None,
            stdout_bytes: 0,
            stderr_bytes: 0,
            idle_deadline,
        }
    }

    pub(super) fn capture(
        &mut self,
        channel: ProcessDataChannel,
        bytes: Vec<u8>,
        settings: &StreamSettings,
    ) -> (Option<ProcessStreamEvent>, Option<ProcessStreamOutcome>) {
        let (emitted, overflowed) = match channel {
            ProcessDataChannel::Stdout | ProcessDataChannel::Pty => {
                capture_bytes(&mut self.stdout_bytes, bytes, settings.stdout_limit)
            }
            ProcessDataChannel::Stderr => {
                capture_bytes(&mut self.stderr_bytes, bytes, settings.stderr_limit)
            }
        };
        let outcome = overflowed.then_some(match channel {
            ProcessDataChannel::Stdout | ProcessDataChannel::Pty => {
                ProcessStreamOutcome::StdoutOverflow
            }
            ProcessDataChannel::Stderr => ProcessStreamOutcome::StderrOverflow,
        });
        let event = (!emitted.is_empty()).then_some(match channel {
            ProcessDataChannel::Stdout | ProcessDataChannel::Pty => {
                ProcessStreamEvent::Stdout(emitted)
            }
            ProcessDataChannel::Stderr => ProcessStreamEvent::Stderr(emitted),
        });
        (event, outcome)
    }

    pub(super) fn deadline_outcome(&self) -> ProcessStreamOutcome {
        self.timeout_outcome(ProcessStreamOutcome::DeadlineExpired)
    }

    pub(super) fn idle_outcome(&self) -> ProcessStreamOutcome {
        self.timeout_outcome(ProcessStreamOutcome::IdleTimeout)
    }

    /// A decoded process end requires a verified trailer, so later timer
    /// expiry represents incomplete transport rather than command execution.
    fn timeout_outcome(&self, outcome: ProcessStreamOutcome) -> ProcessStreamOutcome {
        if self.ended {
            ProcessStreamOutcome::TransportFailure
        } else {
            outcome
        }
    }
}

fn capture_bytes(total: &mut usize, bytes: Vec<u8>, limit: usize) -> (Vec<u8>, bool) {
    let remaining = limit.saturating_sub(*total);
    let emitted = bytes[..bytes.len().min(remaining)].to_vec();
    *total = total.saturating_add(emitted.len());
    (emitted, bytes.len() > remaining)
}

pub(super) async fn deliver(
    event: ProcessStreamEvent,
    settings: &StreamSettings,
    sender: &Sender<ProcessStreamEvent>,
    state: &StreamState,
    outcome: &mut watch::Receiver<Option<ProcessStreamOutcome>>,
) -> Delivery {
    tokio::select! {
        biased;
        _ = sender.closed() => Delivery::ConsumerDropped,
        _ = tokio::time::sleep_until(settings.absolute_deadline) => {
            Delivery::Outcome(state.deadline_outcome())
        }
        _ = outcome.changed() => {
            Delivery::Outcome((*outcome.borrow()).unwrap_or(ProcessStreamOutcome::TransportFailure))
        }
        result = sender.send(event) => match result {
            Ok(()) => Delivery::Sent,
            Err(_) => Delivery::ConsumerDropped,
        },
    }
}

/// Publishes the terminal outcome without waiting for event-queue capacity.
pub(super) fn finish_stream(
    _sender: Sender<ProcessStreamEvent>,
    outcome_sender: oneshot::Sender<StreamTerminal>,
    completion: Completion,
    final_output: Option<ProcessStreamEvent>,
) {
    if let Completion::Outcome(outcome) = completion {
        let _result = outcome_sender.send(StreamTerminal {
            final_output,
            outcome,
        });
    }
}

pub(super) enum Completion {
    Outcome(ProcessStreamOutcome),
    ConsumerDropped,
}

pub(super) enum EventResult {
    Continue,
    Outcome(ProcessStreamOutcome),
    Complete(Completion),
}

impl EventResult {
    pub(super) fn transport_failure() -> Self {
        Self::Outcome(ProcessStreamOutcome::TransportFailure)
    }
}

pub(super) enum Delivery {
    Sent,
    ConsumerDropped,
    Outcome(ProcessStreamOutcome),
}
