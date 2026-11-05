//! Definitive start-code mapping with exact provider retry timing.

use std::time::Duration;

use chrono::{DateTime, Utc};
use sandbox_interface::{
    BillingRefusalReason, PlanSizeRefusalReason, SandboxStartRefusal, SandboxStartRetry,
    TemporaryStartRefusalReason,
};
use serde::Deserialize;

use crate::transport::http::BoatHttpResponse;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEnvelope {
    ok: bool,
    #[serde(rename = "type")]
    kind: String,
    status: u16,
    code: String,
    retry_after_seconds: Option<u64>,
    next_start_at: Option<String>,
    error: Option<ErrorDetail>,
}

#[derive(Deserialize)]
struct ErrorDetail {
    code: String,
    status: u16,
    details: Option<RetryDetail>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RetryDetail {
    retry_after_seconds: Option<u64>,
    next_start_at: Option<String>,
}

pub(super) fn start_refusal(response: &BoatHttpResponse) -> Option<SandboxStartRefusal> {
    let error: ErrorEnvelope = serde_json::from_slice(&response.body).ok()?;
    if error.ok
        || error.kind != "sandbox.error"
        || error.status != response.status
        || error
            .error
            .as_ref()
            .is_some_and(|detail| detail.code != error.code || detail.status != error.status)
    {
        return None;
    }
    let temporary = match (response.status, error.code.as_str()) {
        (429, "rate_limited") => Some(TemporaryStartRefusalReason::RateLimited),
        (429, "limit_reached") => Some(TemporaryStartRefusalReason::AccountConcurrencyLimit),
        (429, "member_limit_reached") => Some(TemporaryStartRefusalReason::MemberConcurrencyLimit),
        (503, "no_ready_machine") => Some(TemporaryStartRefusalReason::NoReadyMachine),
        (503, "out_of_capacity") => Some(TemporaryStartRefusalReason::OutOfCapacity),
        _ => None,
    };
    if let Some(reason) = temporary {
        return Some(SandboxStartRefusal::Temporary {
            reason,
            retry: retry(&error, response.retry_after.as_deref()),
        });
    }
    match (response.status, error.code.as_str()) {
        (403, "machine_class_plan_required") => Some(SandboxStartRefusal::PlanSize {
            reason: PlanSizeRefusalReason::PlanRequired,
        }),
        (403, "trial_machine_class_not_allowed") => Some(SandboxStartRefusal::PlanSize {
            reason: PlanSizeRefusalReason::TrialSizeNotAllowed,
        }),
        (402, "billing_required") => Some(SandboxStartRefusal::Billing {
            reason: BillingRefusalReason::BillingRequired,
        }),
        (402, "team_member_cap_reached") => Some(SandboxStartRefusal::Billing {
            reason: BillingRefusalReason::TeamMemberCapReached,
        }),
        (429, "trial_compute_limit_reached") => Some(SandboxStartRefusal::Billing {
            reason: BillingRefusalReason::TrialComputeLimitReached,
        }),
        (400, "trial_auto_stop_required") => Some(SandboxStartRefusal::TrialAutoStopRequired),
        _ => None,
    }
}

fn retry(error: &ErrorEnvelope, header: Option<&str>) -> SandboxStartRetry {
    let details = error
        .error
        .as_ref()
        .and_then(|error| error.details.as_ref());
    let retry_after = error
        .retry_after_seconds
        .or_else(|| details.and_then(|details| details.retry_after_seconds))
        .or_else(|| header.and_then(|value| value.parse().ok()))
        .map(Duration::from_secs);
    let next_start_at = error
        .next_start_at
        .as_deref()
        .or_else(|| details.and_then(|details| details.next_start_at.as_deref()))
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .or_else(|| header.and_then(|value| DateTime::parse_from_rfc2822(value).ok()))
        .map(|value| value.with_timezone(&Utc));
    SandboxStartRetry {
        retry_after,
        next_start_at,
    }
}
