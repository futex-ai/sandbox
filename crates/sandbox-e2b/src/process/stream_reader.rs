//! Bounded, timestamped provider reads independent of consumer delivery.

use std::time::Duration;

use futures_util::StreamExt;
use sandbox_interface::{ProcessStreamEvent, ProcessStreamOutcome};
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
    stream_drop::drop_grace_deadline,
};

const STAGED_FRAGMENT_BYTES: usize = 64 * 1024;
const STAGED_FRAGMENT_CAPACITY: usize = 32;
const MAX_FRAME_BYTES: usize = 1024 * 1024;

pub(super) enum StagedEvent {
    Process(ProcessEvent),
    Trailer,
    Failure,
}

/// First decoded process identity and whether its end was decoded, published
/// for the entire decoded batch before any events are staged or delivered.
#[derive(Clone, Copy, Default)]
pub(super) struct ReaderObservation {
    pub(super) pid: Option<u32>,
    pub(super) ended: bool,
}

/// Arrival-based timing and bounded PID-discovery grace for one provider reader.
pub(super) struct ReaderTiming {
    pub(super) absolute_deadline: Instant,
    pub(super) idle_deadline: Instant,
    pub(super) idle_timeout: Duration,
    pub(super) discovery_deadline: Option<Instant>,
}

pub(super) struct BufferedReader {
    pub(super) events: Receiver<StagedEvent>,
    pub(super) outcome: watch::Receiver<Option<ProcessStreamOutcome>>,
    /// Keeps outcome polling pending after an ordinary provider EOF.
    _outcome_sender: watch::Sender<Option<ProcessStreamOutcome>>,
    worker: JoinHandle<()>,
}

impl BufferedReader {
    /// Stop decoding before cleanup decides whether a decoded end prevents a kill.
    pub(super) async fn stop(&mut self) {
        self.worker.abort();
        let _ = (&mut self.worker).await;
    }

    pub(super) fn new(
        stream: ByteStream,
        timing: ReaderTiming,
        observed: watch::Sender<ReaderObservation>,
        consumer: Sender<ProcessStreamEvent>,
    ) -> Self {
        let (sender, events) = mpsc::channel(STAGED_FRAGMENT_CAPACITY);
        let (outcome_sender, outcome) = watch::channel(None);
        let keepalive = outcome_sender.clone();
        let worker = tokio::spawn(async move {
            let absolute_deadline = timing.absolute_deadline;
            let finished =
                Self::read(stream, sender, &outcome_sender, &observed, consumer, timing).await;
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
        observed: &watch::Sender<ReaderObservation>,
        consumer: Sender<ProcessStreamEvent>,
        timing: ReaderTiming,
    ) -> Option<Instant> {
        let ReaderTiming {
            absolute_deadline,
            mut idle_deadline,
            idle_timeout,
            mut discovery_deadline,
        } = timing;
        let mut decoder = FrameDecoder::new(MAX_FRAME_BYTES);
        let mut ended = false;
        loop {
            let idle_or_grace_deadline = discovery_deadline.unwrap_or(idle_deadline);
            let item = tokio::select! {
                biased;
                _ = sender.closed() => return None,
                _ = consumer.closed(), if discovery_deadline.is_none() => {
                    discovery_deadline = Some(drop_grace_deadline(absolute_deadline));
                    continue;
                }
                _ = tokio::time::sleep_until(absolute_deadline) => {
                    let result = if ended { ProcessStreamOutcome::TransportFailure } else {
                        ProcessStreamOutcome::DeadlineExpired
                    };
                    let _ = outcome.send(Some(result));
                    return None;
                }
                _ = tokio::time::sleep_until(idle_or_grace_deadline) => {
                    if discovery_deadline.is_some() {
                        return None;
                    }
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
                let mut events = Vec::with_capacity(decoded.frames.len());
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
                    let failed = matches!(event, StagedEvent::Failure);
                    events.push(event);
                    if failed {
                        break;
                    }
                }
                if events.iter().any(|event| {
                    matches!(
                        event,
                        StagedEvent::Process(ProcessEvent::Start(_) | ProcessEvent::End { .. })
                    )
                }) {
                    observed.send_modify(|observation| {
                        for event in &events {
                            match event {
                                StagedEvent::Process(ProcessEvent::Start(pid))
                                    if observation.pid.is_none() =>
                                {
                                    observation.pid = Some(*pid);
                                }
                                StagedEvent::Process(ProcessEvent::End { .. })
                                    if observation.pid.is_some() =>
                                {
                                    observation.ended = true;
                                }
                                _ => {}
                            }
                        }
                    });
                }
                for event in events {
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
