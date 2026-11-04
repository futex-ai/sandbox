//! Independent bounded provider reader, arrival timing, and exact cleanup identity.

use futures_util::StreamExt;
use sandbox_interface::{ProcessStreamEvent, ProcessStreamOutcome};
use tokio::{
    sync::{mpsc::Sender, watch},
    time::Instant,
};

use crate::{
    runtime::{
        api::{AttachRequest, DetachedProcess},
        configured::BoatTrustedRuntime,
    },
    trusted_runtime::attempt::Attempt,
};

use super::{
    framing::Decoder,
    launch::LaunchLease,
    lines::Lines,
    settings::{Counts, Observation, Settings},
    wait::Boundary,
};

pub(super) struct Terminal {
    pub(super) outcome: ProcessStreamOutcome,
    pub(super) prefix: Option<ProcessStreamEvent>,
}

pub(super) struct Reader {
    pub(super) attempt: Attempt,
    pub(super) launch: LaunchLease,
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
    pub(super) async fn read(&mut self) -> ProcessStreamOutcome {
        if self.consumer.is_closed() {
            return ProcessStreamOutcome::TransportFailure;
        }
        let api = self.attempt.api.clone();
        let sandbox = self.attempt.sandbox.clone();
        let timeout = match self.attempt.remaining() {
            Ok(value) => value,
            Err(_) => return self.expired(false),
        };
        let budget = self.attempt.budget;
        let command = self.attempt.command("supervise");
        let opening = async { budget.classify(api.start(&sandbox, command, timeout).await) };
        tokio::pin!(opening);
        let outer = match self.wait(opening.as_mut(), Boundary::Provider).await {
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
            let opening = async { budget.classify(api.attach(request).await) };
            tokio::pin!(opening);
            let mut stream = match self.wait(opening.as_mut(), Boundary::Provider).await {
                Ok(value) => value,
                Err(outcome) => return outcome,
            };
            let mut lines = Lines::default();
            loop {
                let next = async {
                    let result = match stream.next().await {
                        Some(Ok(bytes)) => Ok(Some(bytes)),
                        Some(Err(error)) => Err(error),
                        None => Ok(None),
                    };
                    budget.classify(result)
                };
                tokio::pin!(next);
                let bytes = match self.wait(next.as_mut(), Boundary::Provider).await {
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
                        let mut launch = self.launch.take();
                        let result = {
                            let release = launch.release();
                            tokio::pin!(release);
                            self.wait(release.as_mut(), Boundary::Release).await
                        };
                        self.launch = launch;
                        match result {
                            Ok(output) if BoatTrustedRuntime::helper_succeeded(&output) => {
                                released = true
                            }
                            Ok(_) => return ProcessStreamOutcome::TransportFailure,
                            Err(outcome) => return outcome,
                        }
                    }
                    if batch.success {
                        let attempt = self.attempt.clone();
                        let ack = attempt.run("ack");
                        tokio::pin!(ack);
                        match self.wait(ack.as_mut(), Boundary::Provider).await {
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
}
