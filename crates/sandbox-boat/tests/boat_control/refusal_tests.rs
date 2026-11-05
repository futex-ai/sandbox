//! Definitive start refusals and provider-supplied timing.

use std::time::Duration;

use chrono::{DateTime, Utc};
use sandbox_interface::{
    BillingRefusalReason, Error, PlanSizeRefusalReason, SandboxLifecycle, SandboxStartRefusal,
    SandboxStartRetry, TemporaryStartRefusalReason,
};
use serde_json::json;

use super::support::{
    fixture::Fixture,
    requests,
    state::{self, Override},
};

#[tokio::test]
async fn documented_status_and_code_pairs_remain_distinct_typed_refusals() {
    let fixture = Fixture::start().await;
    let temporary = |reason| SandboxStartRefusal::Temporary {
        reason,
        retry: SandboxStartRetry::default(),
    };
    let cases = [
        (
            429,
            "rate_limited",
            temporary(TemporaryStartRefusalReason::RateLimited),
        ),
        (
            429,
            "limit_reached",
            temporary(TemporaryStartRefusalReason::AccountConcurrencyLimit),
        ),
        (
            429,
            "member_limit_reached",
            temporary(TemporaryStartRefusalReason::MemberConcurrencyLimit),
        ),
        (
            503,
            "no_ready_machine",
            temporary(TemporaryStartRefusalReason::NoReadyMachine),
        ),
        (
            503,
            "out_of_capacity",
            temporary(TemporaryStartRefusalReason::OutOfCapacity),
        ),
        (
            403,
            "machine_class_plan_required",
            SandboxStartRefusal::PlanSize {
                reason: PlanSizeRefusalReason::PlanRequired,
            },
        ),
        (
            403,
            "trial_machine_class_not_allowed",
            SandboxStartRefusal::PlanSize {
                reason: PlanSizeRefusalReason::TrialSizeNotAllowed,
            },
        ),
        (
            402,
            "billing_required",
            SandboxStartRefusal::Billing {
                reason: BillingRefusalReason::BillingRequired,
            },
        ),
        (
            402,
            "team_member_cap_reached",
            SandboxStartRefusal::Billing {
                reason: BillingRefusalReason::TeamMemberCapReached,
            },
        ),
        (
            429,
            "trial_compute_limit_reached",
            SandboxStartRefusal::Billing {
                reason: BillingRefusalReason::TrialComputeLimitReached,
            },
        ),
        (
            400,
            "trial_auto_stop_required",
            SandboxStartRefusal::TrialAutoStopRequired,
        ),
    ];
    for (status, code, expected) in cases {
        fixture.state.lock().unwrap().overrides.push_back(Override {
            method: "POST".to_owned(),
            path: "/api/v1/sandboxes".to_owned(),
            reply: state::error(status, code),
        });
        let result = fixture.backend.create_sandbox(requests::create()).await;
        let Err(Error::SandboxStartRefused {
            backend_id,
            refusal,
        }) = result
        else {
            panic!("expected definitive refusal for {status} {code}, got {result:?}");
        };
        assert_eq!(backend_id, "boat");
        assert_eq!(refusal, expected, "{status} {code}");
    }
    assert_eq!(fixture.captured().len(), 11);
}

#[tokio::test]
async fn temporary_refusals_preserve_body_seconds_utc_and_both_retry_after_forms() {
    let fixture = Fixture::start().await;
    let next: DateTime<Utc> = "2026-11-04T12:00:00Z".parse().unwrap();
    for (body, header, expected) in [
        (
            json!({"retryAfterSeconds":0,"nextStartAt":"2026-11-04T12:00:00Z"}),
            Some("9"),
            SandboxStartRetry {
                retry_after: Some(Duration::ZERO),
                next_start_at: Some(next),
            },
        ),
        (
            json!({}),
            Some("17"),
            SandboxStartRetry {
                retry_after: Some(Duration::from_secs(17)),
                next_start_at: None,
            },
        ),
        (
            json!({}),
            Some("Wed, 04 Nov 2026 12:00:00 GMT"),
            SandboxStartRetry {
                retry_after: None,
                next_start_at: Some(next),
            },
        ),
        (json!({}), None, SandboxStartRetry::default()),
    ] {
        let mut reply = state::error(429, "rate_limited");
        let mut envelope: serde_json::Value = serde_json::from_slice(&reply.body).unwrap();
        envelope
            .as_object_mut()
            .unwrap()
            .extend(body.as_object().unwrap().clone());
        reply.body = serde_json::to_vec(&envelope).unwrap();
        if let Some(header) = header {
            reply
                .headers
                .push(("Retry-After".to_owned(), header.to_owned()));
        }
        fixture.state.lock().unwrap().overrides.push_back(Override {
            method: "POST".to_owned(),
            path: "/api/v1/sandboxes".to_owned(),
            reply,
        });
        let result = fixture.backend.create_sandbox(requests::create()).await;
        let Err(Error::SandboxStartRefused {
            refusal: SandboxStartRefusal::Temporary { retry, .. },
            ..
        }) = result
        else {
            panic!("unexpected result {result:?}");
        };
        assert_eq!(retry, expected);
    }
}

#[tokio::test]
async fn uncertain_responses_and_status_mismatches_never_prove_a_refused_start() {
    let fixture = Fixture::start().await;
    for (status, code) in [
        (500, "out_of_capacity"),
        (503, "billing_required"),
        (403, "rate_limited"),
        (429, "unknown_start_error"),
    ] {
        fixture.state.lock().unwrap().overrides.push_back(Override {
            method: "POST".to_owned(),
            path: "/api/v1/sandboxes".to_owned(),
            reply: state::error(status, code),
        });
        assert!(
            matches!(
                fixture.backend.create_sandbox(requests::create()).await,
                Err(Error::BackendUnavailable { .. })
            ),
            "{status} {code}"
        );
    }
    assert_eq!(
        fixture
            .captured()
            .iter()
            .filter(|call| call.method == "GET")
            .count(),
        0
    );
}
