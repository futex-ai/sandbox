//! Phase-aware waits preserve deadline, idle, and bounded PID-discovery outcomes.

use std::{
    future::{Future, poll_fn},
    pin::Pin,
    task::Poll,
    time::Duration,
};

use sandbox_interface::ProcessStreamOutcome;
use tokio::time::Instant;

use crate::trusted_runtime::budget::{StepError, StepResult};

use super::reader::Reader;

/// Unpolled release must not run during a consumer-drop discovery grace.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Boundary {
    /// Provider reads can discover a late child after release.
    Provider,
    /// The first release poll can permit caller code to start.
    Release,
}

impl Reader {
    pub(super) async fn wait<T>(
        &mut self,
        mut future: Pin<&mut (dyn Future<Output = StepResult<T>> + Send)>,
        boundary: Boundary,
    ) -> std::result::Result<T, ProcessStreamOutcome> {
        let mut polled = false;
        loop {
            if self.dropped.is_some() && self.observed.borrow().pid.is_some() {
                return Err(ProcessStreamOutcome::TransportFailure);
            }
            let cutoff = self
                .dropped
                .unwrap_or(self.idle)
                .min(self.settings.deadline);
            let expired = self.expired(cutoff != self.settings.deadline);
            let absolute_expired = self.expired(false);
            tokio::select! {
                biased;
                _ = self.consumer.closed(), if self.dropped.is_none() => {
                    if self.launch.armed() || !self.polled || (boundary == Boundary::Release && !polled) {
                        return Err(ProcessStreamOutcome::TransportFailure);
                    }
                    self.dropped = Some((Instant::now() + Duration::from_secs(3)).min(self.settings.deadline));
                }
                _ = tokio::time::sleep_until(cutoff) => {
                    return Err(expired);
                }
                value = poll_fn(|cx| {
                    if Instant::now() >= cutoff {
                        return Poll::Ready(Err(expired));
                    }
                    if boundary == Boundary::Release && !polled && self.consumer.is_closed() {
                        return Poll::Ready(Err(ProcessStreamOutcome::TransportFailure));
                    }
                    polled = true;
                    self.polled = true;
                    self.launch.opening();
                    future.as_mut().poll(cx).map(|result| match result {
                        Ok(value) => Ok(value),
                        Err(StepError::Expired) => Err(absolute_expired),
                        Err(StepError::Failed(_)) => Err(ProcessStreamOutcome::TransportFailure),
                    })
                }) => {
                    return value;
                }
            }
        }
    }

    pub(super) fn expired(&self, idle: bool) -> ProcessStreamOutcome {
        if self.observed.borrow().ended || self.dropped.is_some() {
            ProcessStreamOutcome::TransportFailure
        } else if idle {
            ProcessStreamOutcome::IdleTimeout
        } else {
            ProcessStreamOutcome::DeadlineExpired
        }
    }
}
