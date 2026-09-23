//! Bounded PID discovery after a streaming consumer disconnects.

use tokio::{sync::watch, time::Instant};

use super::{
    helper_run::KILL_DEADLINE,
    stream_reader::{BufferedReader, ReaderObservation},
};

pub(super) fn drop_grace_deadline(absolute_deadline: Instant) -> Instant {
    Instant::now()
        .checked_add(KILL_DEADLINE)
        .map_or(absolute_deadline, |deadline| {
            deadline.min(absolute_deadline)
        })
}

/// Keep the provider reader alive until it observes an identity, reaches EOF,
/// or exhausts the drop grace; discard events that can no longer be delivered.
pub(super) async fn await_pid_on_drop(
    reader: &mut BufferedReader,
    observed: &mut watch::Receiver<ReaderObservation>,
    deadline: Instant,
) {
    loop {
        if observed.borrow_and_update().pid.is_some() {
            return;
        }
        tokio::select! {
            biased;
            _ = tokio::time::sleep_until(deadline) => break,
            result = reader.events.recv() => {
                if result.is_none() {
                    break;
                }
            }
            result = observed.changed() => {
                if result.is_err() {
                    break;
                }
            }
        }
    }
    tracing::debug!(event = "e2b_stream_drop_pid_unobserved");
}
