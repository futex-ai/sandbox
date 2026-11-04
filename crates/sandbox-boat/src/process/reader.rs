//! Independent bounded provider reader, arrival timing, and exact cleanup identity.

use std::{
    future::{Future, poll_fn},
    pin::Pin,
    time::Duration,
};

use futures_util::StreamExt;
use sandbox_interface::{ProcessStreamEvent, ProcessStreamOutcome};
use tokio::{
    sync::{mpsc::Sender, watch},
    time::Instant,
};

use crate::{
    error::Result,
    runtime::{
        api::{AttachRequest, DetachedProcess},
        configured::BoatTrustedRuntime,
    },
    trusted_runtime::attempt::Attempt,
};

use super::{
    framing::Decoder,
    lines::Lines,
    settings::{Counts, Observation, Settings},
};

pub(super) struct Terminal {
    pub(super) outcome: ProcessStreamOutcome,
    pub(super) prefix: Option<ProcessStreamEvent>,
}

pub(super) struct Reader {
    pub(super) attempt: Attempt,
    pub(super) settings: Settings,
    pub(super) staged: Sender<ProcessStreamEvent>,
    pub(super) consumer: Sender<ProcessStreamEvent>,
    pub(super) observed: watch::Sender<Observation>,
    pub(super) idle: Instant,
    pub(super) dropped: Option<Instant>,
    pub(super) outer: Option<DetachedProcess>,
    pub(super) polled: bool,
    pub(super) prefix: Option<ProcessStreamEvent>,
}

impl Reader {
    pub(super) async fn run(mut self) -> Terminal {
        let outcome = self.read().await;
        let observation = *self.observed.borrow();
        if self.consumer.is_closed() && observation.pid.is_none() {
            return Terminal {
                outcome,
                prefix: self.prefix.take(),
            };
        }
        let resolved;
        if !observation.ended {
            resolved = self
                .attempt
                .cleanup("terminate")
                .await
                .as_ref()
                .is_ok_and(BoatTrustedRuntime::helper_succeeded);
            if let Some(outer) = self.outer {
                let api = self.attempt.api.clone();
                let stop = api.stop(
                    &self.attempt.sandbox,
                    outer.process_id,
                    Duration::from_secs(2),
                );
                let _ = tokio::time::timeout(Duration::from_secs(2), stop).await;
            }
        } else {
            resolved = self
                .attempt
                .cleanup("ack")
                .await
                .as_ref()
                .is_ok_and(BoatTrustedRuntime::helper_succeeded);
        }
        if outcome == ProcessStreamOutcome::Completed || resolved {
            let _ = self.attempt.cleanup("clean").await;
        }
        Terminal {
            outcome,
            prefix: self.prefix.take(),
        }
    }

    async fn read(&mut self) -> ProcessStreamOutcome {
        if self.consumer.is_closed() {
            return ProcessStreamOutcome::TransportFailure;
        }
        let api = self.attempt.api.clone();
        let sandbox = self.attempt.sandbox.clone();
        let timeout = match self.attempt.remaining() {
            Ok(value) => value,
            Err(_) => return self.expired(false),
        };
        let opening = api.start(&sandbox, self.attempt.command("supervise"), timeout);
        tokio::pin!(opening);
        let outer = match self.wait(opening.as_mut()).await {
            Ok(value) => value,
            Err(outcome) => return outcome,
        };
        self.outer = Some(outer);
        let mut decoder = Decoder::new(outer.process_id, self.attempt.id.clone());
        let mut counts = Counts::new();
        let mut released = false;
        loop {
            let timeout = match self.attempt.remaining() {
                Ok(value) => value,
                Err(_) => return self.expired(false),
            };
            let request = AttachRequest {
                sandbox: sandbox.clone(),
                process: outer.process_id,
                stdout: decoder.stdout,
                stderr: decoder.stderr,
                timeout,
            };
            let opening = api.attach(request);
            tokio::pin!(opening);
            let mut stream = match self.wait(opening.as_mut()).await {
                Ok(value) => value,
                Err(outcome) => return outcome,
            };
            let mut lines = Lines::default();
            loop {
                let next = async {
                    match stream.next().await {
                        Some(Ok(bytes)) => Ok(Some(bytes)),
                        Some(Err(error)) => Err(error),
                        None => Ok(None),
                    }
                };
                tokio::pin!(next);
                let bytes = match self.wait(next.as_mut()).await {
                    Ok(Some(bytes)) => bytes,
                    Ok(None) => break,
                    Err(outcome) => return outcome,
                };
                let arrived = Instant::now();
                let mut bytes = bytes.as_slice();
                while !bytes.is_empty() {
                    let line = match lines.take(&mut bytes, 1024 * 1024) {
                        Ok(Some(line)) => line,
                        Ok(None) => break,
                        Err(_) => return ProcessStreamOutcome::TransportFailure,
                    };
                    let batch = match decoder.outer(&line) {
                        Ok(batch) => batch,
                        Err(_) => return ProcessStreamOutcome::TransportFailure,
                    };
                    self.observed.send_modify(|value| {
                        for event in &batch.events {
                            match event {
                                ProcessStreamEvent::Started { pid } if value.pid.is_none() => {
                                    value.pid = Some(*pid)
                                }
                                ProcessStreamEvent::Exited { .. } if value.pid.is_some() => {
                                    value.ended = true
                                }
                                _ => {}
                            }
                        }
                    });
                    for mut event in batch.events {
                        if matches!(&event, ProcessStreamEvent::Stdout(b) | ProcessStreamEvent::Stderr(b) if !b.is_empty())
                        {
                            let Some(idle) = arrived.checked_add(self.settings.idle_timeout) else {
                                return ProcessStreamOutcome::TransportFailure;
                            };
                            self.idle = idle;
                        }
                        if let Some(outcome) = counts.capture(&mut event, &self.settings) {
                            self.prefix = Some(event);
                            return outcome;
                        }
                        if self.staged.try_send(event).is_err() {
                            return ProcessStreamOutcome::ConsumerBackpressure;
                        }
                        tokio::task::yield_now().await;
                    }
                    if batch.failure {
                        return ProcessStreamOutcome::TransportFailure;
                    }
                    if decoder.attached && !released {
                        if self.consumer.is_closed() {
                            return ProcessStreamOutcome::TransportFailure;
                        }
                        let attempt = self.attempt.clone();
                        let release = attempt.run("release");
                        tokio::pin!(release);
                        match self.wait(release.as_mut()).await {
                            Ok(output) if BoatTrustedRuntime::helper_succeeded(&output) => {
                                released = true
                            }
                            _ => return ProcessStreamOutcome::TransportFailure,
                        }
                    }
                    if batch.success {
                        let attempt = self.attempt.clone();
                        let ack = attempt.run("ack");
                        tokio::pin!(ack);
                        match self.wait(ack.as_mut()).await {
                            Ok(output) if BoatTrustedRuntime::helper_succeeded(&output) => {}
                            _ => return ProcessStreamOutcome::TransportFailure,
                        }
                    }
                }
            }
            if !lines.empty() {
                return ProcessStreamOutcome::TransportFailure;
            }
            match decoder.terminal {
                Some(true) => return ProcessStreamOutcome::Completed,
                Some(false) => {
                    if decoder.reconnect().is_err() {
                        return ProcessStreamOutcome::TransportFailure;
                    }
                }
                _ => return ProcessStreamOutcome::TransportFailure,
            }
        }
    }

    async fn wait<T>(
        &mut self,
        mut future: Pin<&mut (dyn Future<Output = Result<T>> + Send)>,
    ) -> std::result::Result<T, ProcessStreamOutcome> {
        loop {
            if self.dropped.is_some() && self.observed.borrow().pid.is_some() {
                return Err(ProcessStreamOutcome::TransportFailure);
            }
            let cutoff = self
                .dropped
                .unwrap_or(self.idle)
                .min(self.settings.deadline);
            tokio::select! {
                biased;
                _ = self.consumer.closed(), if self.dropped.is_none() => {
                    if !self.polled { return Err(ProcessStreamOutcome::TransportFailure); }
                    self.dropped = Some((Instant::now() + Duration::from_secs(3)).min(self.settings.deadline));
                }
                _ = tokio::time::sleep_until(cutoff) => {
                    return Err(self.expired(cutoff != self.settings.deadline));
                }
                value = poll_fn(|cx| { self.polled = true; future.as_mut().poll(cx) }) => {
                    return match value { Ok(value) => Ok(value), Err(_) => Err(ProcessStreamOutcome::TransportFailure) };
                }
            }
        }
    }

    fn expired(&self, idle: bool) -> ProcessStreamOutcome {
        if self.observed.borrow().ended || self.dropped.is_some() {
            ProcessStreamOutcome::TransportFailure
        } else if idle {
            ProcessStreamOutcome::IdleTimeout
        } else {
            ProcessStreamOutcome::DeadlineExpired
        }
    }
}
