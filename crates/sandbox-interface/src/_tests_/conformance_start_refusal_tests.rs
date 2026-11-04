//! Definitive refusals must not become a recovered or queued start.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use chrono::{TimeZone, Utc};
use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use crate::conformance_resources::ConformanceResources;
use crate::{
    BackendCreateSandboxRequest, BackendSandbox, BillingRefusalReason, Error, OperationId,
    PlanSizeRefusalReason, ProviderRef, ResourceOwner, SandboxConsumer, SandboxId,
    SandboxLifecycleMock, SandboxStartRefusal, SandboxStartRetry, SandboxState,
    TemporaryStartRefusalReason,
};

#[tokio::test]
async fn definitive_start_refusals_retain_details_without_recovery() {
    let refusals = [
        SandboxStartRefusal::Temporary {
            reason: TemporaryStartRefusalReason::RateLimited,
            retry: SandboxStartRetry {
                retry_after: Some(Duration::from_secs(7)),
                next_start_at: Some(Utc.with_ymd_and_hms(2026, 11, 4, 12, 0, 7).unwrap()),
            },
        },
        SandboxStartRefusal::Temporary {
            reason: TemporaryStartRefusalReason::AccountConcurrencyLimit,
            retry: SandboxStartRetry::default(),
        },
        SandboxStartRefusal::PlanSize {
            reason: PlanSizeRefusalReason::PlanRequired,
        },
        SandboxStartRefusal::Billing {
            reason: BillingRefusalReason::BillingRequired,
        },
        SandboxStartRefusal::TrialAutoStopRequired,
    ];

    for refusal in refusals {
        let recoveries = Arc::new(AtomicUsize::new(0));
        let creates = Arc::new(AtomicUsize::new(0));
        let backend = Unimock::new((
            SandboxLifecycleMock::create_sandbox
                .some_call(matching!(_))
                .answers_arc({
                    let creates = creates.clone();
                    let refusal = refusal.clone();
                    Arc::new(move |_, _| {
                        creates.fetch_add(1, Ordering::SeqCst);
                        Err(Error::SandboxStartRefused {
                            backend_id: "provider".to_owned(),
                            refusal: refusal.clone(),
                        })
                    })
                }),
            SandboxLifecycleMock::recover_sandbox_create
                .each_call(matching!(_))
                .answers_arc({
                    let recoveries = recoveries.clone();
                    Arc::new(move |_, _| {
                        recoveries.fetch_add(1, Ordering::SeqCst);
                        Ok(Some(BackendSandbox {
                            provider_ref: ProviderRef::new("unrelated-existing"),
                            state: SandboxState::Ready,
                        }))
                    })
                })
                .at_least_times(0),
        ))
        .no_verify_in_drop();
        let sleeper = Unimock::new(());
        let mut resources = ConformanceResources::new(&sleeper);

        let result = resources.create_sandbox(&backend, request()).await;

        assert!(
            matches!(result, Err(Error::SandboxStartRefused {
            ref backend_id, refusal: ref actual,
        }) if backend_id == "provider" && actual == &refusal),
            "refusal lost or recovered: {result:?}"
        );
        resources
            .cleanup()
            .await
            .expect("a refused start has no resource to clean");
        assert_eq!(creates.load(Ordering::SeqCst), 1);
        assert_eq!(recoveries.load(Ordering::SeqCst), 0);
    }
}

fn request() -> BackendCreateSandboxRequest {
    BackendCreateSandboxRequest {
        sandbox_id: SandboxId::new(),
        operation_id: OperationId::new(),
        owner: ResourceOwner::platform(Uuid::now_v7()),
        consumer: SandboxConsumer::Runtime,
        lifetime: Default::default(),
        size: None,
        deployment_id: "conformance".to_owned(),
        profile: "general".to_owned(),
        network: Default::default(),
        snapshot_provider_ref: None,
    }
}
