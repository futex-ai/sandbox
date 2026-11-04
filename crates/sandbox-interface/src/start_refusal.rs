//! Definitive start refusals, distinct from uncertain mutation delivery.

use std::time::Duration;

use chrono::{DateTime, Utc};
use thiserror::Error;

/// Retry timing supplied by the provider, without an invented delay.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SandboxStartRetry {
    /// Provider-supplied relative delay; retain zero when explicitly supplied.
    pub retry_after: Option<Duration>,
    /// Provider-supplied absolute next permitted start time.
    pub next_start_at: Option<DateTime<Utc>>,
}

/// Why the provider definitively refused to start a sandbox.
///
/// Every variant proves that this start did not run or queue. A transport
/// failure with uncertain delivery must never be converted to this type.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum SandboxStartRefusal {
    /// Capacity, start-window, or concurrent usage currently blocks a start.
    #[error("[sandbox_interface/start_refusal] temporary start refusal: {reason:?}")]
    Temporary {
        /// Typed provider reason for a temporary refusal.
        reason: TemporaryStartRefusalReason,
        /// Exact provider-supplied retry timing.
        retry: SandboxStartRetry,
    },
    /// The account plan or trial does not permit the selected size.
    #[error("[sandbox_interface/start_refusal] plan does not permit size: {reason:?}")]
    PlanSize {
        /// Typed plan or trial restriction.
        reason: PlanSizeRefusalReason,
    },
    /// Billing or account compute allowance blocks the start.
    #[error("[sandbox_interface/start_refusal] billing refusal: {reason:?}")]
    Billing {
        /// Typed billing or allowance restriction.
        reason: BillingRefusalReason,
    },
    /// The trial requires auto-stop within its permitted lifetime.
    #[error("[sandbox_interface/start_refusal] trial requires a permitted auto-stop lifetime")]
    TrialAutoStopRequired,
}

/// Temporary provider reasons that prove no sandbox started.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporaryStartRefusalReason {
    /// Provider start rate or start window was reached.
    RateLimited,
    /// No prewarmed machine is ready.
    NoReadyMachine,
    /// Provider capacity is exhausted.
    OutOfCapacity,
    /// Account concurrency allowance is exhausted.
    AccountConcurrencyLimit,
    /// Member concurrency allowance is exhausted.
    MemberConcurrencyLimit,
}

/// Plan-specific reasons a provider refuses a selected size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanSizeRefusalReason {
    /// The selected machine class requires another plan.
    PlanRequired,
    /// The trial does not permit the selected machine class.
    TrialSizeNotAllowed,
}

/// Billing and account-allowance reasons that block a start.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BillingRefusalReason {
    /// The account must configure or repair billing.
    BillingRequired,
    /// A team member spending cap was reached.
    TeamMemberCapReached,
    /// The trial compute allowance was reached.
    TrialComputeLimitReached,
}
