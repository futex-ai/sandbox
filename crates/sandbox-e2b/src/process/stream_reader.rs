//! Bounded, timestamped provider reads independent of consumer delivery.

use std::time::Duration;

use futures_util::StreamExt;
use sandbox_interface::ProcessStreamOutcome;
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        watch,
    },
    task::JoinHandle,
    time::Instant,
};

use super::{
    framing::{FrameDecoder, ProcessDataChannel, ProcessEvent, decode_end_stream, decode_event},
    http::ByteStream,
};

const STAGED_FRAGMENT_BYTES: usize = 64 * 1024;
const STAGED_FRAGMENT_CAPACITY: usize = 32;
const MAX_FRAME_BYTES: usize = 1024 * 1024;

pub(super) enum StagedEvent {
    Process(ProcessEvent),
    Trailer,
    Failure,
}

pub(super) struct BufferedReader {
    pub(super) events: Receiver<StagedEvent>,
    pub(super) outcome: watch::Receiver<Option<ProcessStreamOutcome>>,
    /// Keeps outcome polling pending after an ordinary provider EOF.
    _outcome_sender: watch::Sender<Option<ProcessStreamOutcome>>,
    worker: JoinHandle<()>,
}

impl BufferedReader {
    pub(super) fn new(
        stream: ByteStream,
        absolute_deadline: Instant,
        idle_deadline: Instant,
        idle_timeout: Duration,
    ) -> Self {
        let (sender, events) = mpsc::channel(STAGED_FRAGMENT_CAPACITY);
        let (outcome_sender, outcome) = watch::channel(None);
        let keepalive = outcome_sender.clone();
        let worker = tokio::spawn(async move {
            let finished = Self::read(
                stream,
                sender,
                &outcome_sender,
                absolute_deadline,
                idle_deadline,
                idle_timeout,
            )
            .await;
            if let Some(idle_deadline) = finished {
                tokio::select! {
                    _ = outcome_sender.closed() => {}
                    _ = tokio::time::sleep_until(absolute_deadline) => {
                        let _ = outcome_sender.send(Some(ProcessStreamOutcome::TransportFailure));
                    }
                    _ = tokio::time::sleep_until(idle_deadline) => {
                        let _ = outcome_sender.send(Some(ProcessStreamOutcome::TransportFailure));
                    }
                }
            }
        });
        Self {
            events,
            outcome,
            _outcome_sender: keepalive,
            worker,
        }
    }

    async fn read(
        mut stream: ByteStream,
        sender: Sender<StagedEvent>,
        outcome: &watch::Sender<Option<ProcessStreamOutcome>>,
        absolute_deadline: Instant,
        mut idle_deadline: Instant,
        idle_timeout: Duration,
    ) -> Option<Instant> {
        let mut decoder = FrameDecoder::new(MAX_FRAME_BYTES);
        let mut ended = false;
        loop {
            let item = tokio::select! {
                biased;
                _ = sender.closed() => return None,
                _ = tokio::time::sleep_until(absolute_deadline) => {
                    let result = if ended { ProcessStreamOutcome::TransportFailure } else {
                        ProcessStreamOutcome::DeadlineExpired
                    };
                    let _ = outcome.send(Some(result));
                    return None;
                }
                _ = tokio::time::sleep_until(idle_deadline) => {
                    let result = if ended { ProcessStreamOutcome::TransportFailure } else {
                        ProcessStreamOutcome::IdleTimeout
                    };
                    let _ = outcome.send(Some(result));
                    return None;
                }
                item = stream.next() => item,
            };
            let Some(item) = item else {
                return Some(idle_deadline);
            };
            let Ok(bytes) = item else {
                Self::stage(&sender, outcome, StagedEvent::Failure);
                return Some(idle_deadline);
            };
            let arrived_at = Instant::now();
            for chunk in bytes.chunks(STAGED_FRAGMENT_BYTES) {
                let decoded = decoder.push(chunk);
                for frame in decoded.frames {
                    let event = if frame.end_stream {
                        if decode_end_stream(&frame.payload).is_ok() {
                            StagedEvent::Trailer
                        } else {
                            StagedEvent::Failure
                        }
                    } else {
                        match decode_event(&frame.payload) {
                            Ok(event) => StagedEvent::Process(event),
                            Err(_) => StagedEvent::Failure,
                        }
                    };
                    if let StagedEvent::Process(ProcessEvent::Data { channel, ref bytes }) = event
                        && !bytes.is_empty()
                        && matches!(
                            channel,
                            ProcessDataChannel::Stdout | ProcessDataChannel::Stderr
                        )
                    {
                        let Some(deadline) = arrived_at.checked_add(idle_timeout) else {
                            Self::stage(&sender, outcome, StagedEvent::Failure);
                            return Some(idle_deadline);
                        };
                        idle_deadline = deadline;
                    }
                    if matches!(event, StagedEvent::Process(ProcessEvent::End { .. })) {
                        ended = true;
                    }
                    let failed = matches!(event, StagedEvent::Failure);
                    if !Self::stage(&sender, outcome, event) {
                        return None;
                    }
                    if failed {
                        return Some(idle_deadline);
                    }
                    tokio::task::yield_now().await;
                }
                if decoded.terminal_error.is_some() {
                    Self::stage(&sender, outcome, StagedEvent::Failure);
                    return Some(idle_deadline);
                }
            }
        }
    }

    fn stage(
        sender: &Sender<StagedEvent>,
        outcome: &watch::Sender<Option<ProcessStreamOutcome>>,
        event: StagedEvent,
    ) -> bool {
        if sender.try_send(event).is_ok() {
            return true;
        }
        let _ = outcome.send(Some(ProcessStreamOutcome::ConsumerBackpressure));
        false
    }
}

impl Drop for BufferedReader {
    fn drop(&mut self) {
        self.worker.abort();
    }
}
