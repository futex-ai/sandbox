//! Separate 32-event staging, 16-event delivery, and queue-independent outcomes.

use futures_util::stream;
use sandbox_interface::{ProcessEventStream, ProcessStreamEvent, ProcessStreamOutcome};
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::Instant,
};

use crate::trusted_runtime::attempt::Attempt;

use super::{
    launch::LaunchLease,
    reader::{Reader, Terminal},
    settings::{Observation, Settings},
};

pub(super) fn events(attempt: Attempt, settings: Settings) -> ProcessEventStream {
    let deadline = settings.deadline;
    let idle_timeout = settings.idle_timeout;
    let (consumer, delivery) = mpsc::channel(16);
    let (staged, staging) = mpsc::channel(32);
    let (terminal_sender, terminal) = oneshot::channel();
    let (observed, _) = watch::channel(Observation::default());
    let launch_lease = LaunchLease::new(attempt.clone());
    let launch = async move {
        let _worker = tokio::spawn(async move {
            let idle = Instant::now().checked_add(idle_timeout).unwrap_or(deadline);
            let reader = Reader {
                attempt,
                launch: launch_lease,
                settings,
                staged,
                consumer: consumer.clone(),
                observed,
                idle,
                dropped: None,
                outer: None,
                prefix: None,
                polled: false,
            };
            let reader_task = tokio::spawn(reader.run());
            let mut staging = staging;
            while let Some(event) = staging.recv().await {
                if consumer.send(event).await.is_err() {
                    break;
                }
            }
            let result = match reader_task.await {
                Ok(terminal) => terminal,
                Err(_) => Terminal {
                    outcome: ProcessStreamOutcome::TransportFailure,
                    prefix: None,
                },
            };
            let _ = terminal_sender.send(result);
        });
    };
    let receiver = Receiver {
        delivery,
        terminal: Some(terminal),
        final_event: None,
        done: false,
    };
    let stream = stream::unfold(
        (receiver, Some(launch)),
        |(mut receiver, launch)| async move {
            if let Some(launch) = launch {
                launch.await;
            }
            receiver.next().await.map(|event| (event, (receiver, None)))
        },
    );
    Box::pin(stream)
}

struct Receiver {
    delivery: mpsc::Receiver<ProcessStreamEvent>,
    terminal: Option<oneshot::Receiver<Terminal>>,
    final_event: Option<ProcessStreamEvent>,
    done: bool,
}

impl Receiver {
    async fn next(&mut self) -> Option<ProcessStreamEvent> {
        if let Some(event) = self.final_event.take() {
            self.done = true;
            return Some(event);
        }
        if self.done {
            return None;
        }
        if let Some(event) = self.delivery.recv().await {
            return Some(event);
        }
        let terminal = match self.terminal.take() {
            Some(receiver) => receiver.await.ok(),
            None => None,
        }
        .unwrap_or(Terminal {
            outcome: ProcessStreamOutcome::TransportFailure,
            prefix: None,
        });
        let outcome = ProcessStreamEvent::Outcome(terminal.outcome);
        if let Some(prefix) = terminal.prefix {
            self.final_event = Some(outcome);
            Some(prefix)
        } else {
            self.done = true;
            Some(outcome)
        }
    }
}
