//! Private absolute-budget classification, independent of provider error shape.

use std::time::Duration;

use tokio::time::Instant;

use crate::error::{Error, Result};

/// A provider step either completes within its budget, expires, or fails.
pub(crate) type StepResult<T> = std::result::Result<T, StepError>;

/// Private timing and provider failures remain distinct until public mapping.
#[derive(Debug, thiserror::Error)]
pub(crate) enum StepError {
    /// The absolute clock consumed the accepted budget.
    #[error("[sandbox-boat/budget] absolute process budget expired")]
    Expired,
    /// The provider failed while budget remained.
    #[error("[sandbox-boat/budget] runtime step failed")]
    Failed(#[source] Error),
}

/// The clock owns expiry classification, including generic HTTP timeout ties.
#[derive(Clone, Copy)]
pub(crate) struct Budget {
    deadline: Instant,
}

impl Budget {
    /// Retain the checked deadline of this attempt.
    pub(crate) fn new(deadline: Instant) -> Self {
        Self { deadline }
    }

    /// Reject dispatch when no execution time remains.
    pub(crate) fn remaining(&self) -> Result<Duration> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            Err(Error::Unavailable)
        } else {
            Ok(remaining)
        }
    }

    /// Check the clock even when the provider reports only a generic failure.
    pub(crate) fn classify<T>(&self, result: Result<T>) -> StepResult<T> {
        if self.remaining().is_err() {
            return Err(StepError::Expired);
        }
        match result {
            Ok(value) => Ok(value),
            Err(error) => Err(StepError::Failed(error)),
        }
    }
}
