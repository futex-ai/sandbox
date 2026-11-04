//! One cleanup lease follows prepared state until launch release can dispatch.

use std::time::Duration;

use crate::{
    error::Error,
    runtime::api::HelperOutput,
    trusted_runtime::{
        attempt::Attempt,
        budget::{StepError, StepResult},
    },
};

/// Before release, dropping any owner removes its prepared attempt state.
pub(super) struct LaunchLease {
    attempt: Option<Attempt>,
    opening: bool,
}

impl LaunchLease {
    /// Own the prepared state before a supervisor can have been dispatched.
    pub(super) fn new(attempt: Attempt) -> Self {
        Self {
            attempt: Some(attempt),
            opening: false,
        }
    }

    /// State may be removed only while this lease still owns it.
    pub(super) fn armed(&self) -> bool {
        self.attempt.is_some()
    }

    /// A polled supervisor start needs a cancellation marker before removal.
    pub(super) fn opening(&mut self) {
        self.opening = true;
    }

    /// Move the sole owner into a release future, leaving no second guard.
    pub(super) fn take(&mut self) -> Self {
        Self {
            attempt: self.attempt.take(),
            opening: self.opening,
        }
    }

    /// Keep ownership on unpolled release or synchronous budget exhaustion.
    /// Transfer ownership before dispatch, since an uncertain reply may hide a child.
    pub(super) async fn release(&mut self) -> StepResult<HelperOutput> {
        let Some(attempt) = self.attempt.clone() else {
            return Err(StepError::Failed(Error::InvalidRequest));
        };
        let command = attempt.command("release");
        let timeout = match attempt.remaining() {
            Ok(timeout) => timeout.min(Duration::from_secs(300)),
            Err(error) => return attempt.budget.classify(Err(error)),
        };
        self.attempt = None;
        let output = attempt.api.helper(&attempt.sandbox, command, timeout).await;
        attempt.budget.classify(output)
    }

    /// Before release, cancel any waiting supervisor and remove prepared state.
    pub(super) async fn cleanup(&mut self) {
        if let Some(attempt) = &self.attempt {
            if self.opening {
                let _ = attempt.cleanup("terminate").await;
            }
            let _ = attempt.cleanup("clean").await;
        }
        self.attempt = None;
    }
}

impl Drop for LaunchLease {
    fn drop(&mut self) {
        if let Some(attempt) = self.attempt.take()
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            let opening = self.opening;
            let _cleanup = runtime.spawn(async move {
                if opening {
                    let _ = attempt.cleanup("terminate").await;
                }
                let _ = attempt.cleanup("clean").await;
            });
        }
    }
}
