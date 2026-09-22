//! State and bounded delivery helpers for process event streaming.

use std::time::Duration;

use sandbox_interface::{ProcessStreamEvent, ProcessStreamOutcome};
use tokio::sync::mpsc::Sender;

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
    stdout_bytes: usize,
    stderr_bytes: usize,
    pub(super) idle_deadline: tokio::time::Instant,
}

impl StreamState {
    pub(super) fn new(idle_deadline: tokio::time::Instant) -> Self {
        Self {
            pid: None,
            started: false,
            ended: false,
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
    idle_deadline: tokio::time::Instant,
) -> Delivery {
    tokio::select! {
        biased;
        _ = sender.closed() => Delivery::ConsumerDropped,
        _ = tokio::time::sleep_until(settings.absolute_deadline) => {
            Delivery::Outcome(ProcessStreamOutcome::DeadlineExpired)
        }
        _ = tokio::time::sleep_until(idle_deadline) => {
            Delivery::Outcome(ProcessStreamOutcome::IdleTimeout)
        }
        result = sender.send(event) => match result {
            Ok(()) => Delivery::Sent,
            Err(_) => Delivery::ConsumerDropped,
        },
    }
}

/// Sends the terminal outcome and consumes the last worker-side sender.
pub(super) async fn finish_stream(sender: Sender<ProcessStreamEvent>, completion: Completion) {
    if let Completion::Outcome(outcome) = completion {
        let _result = sender.send(ProcessStreamEvent::Outcome(outcome)).await;
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
