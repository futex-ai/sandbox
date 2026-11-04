//! Explicit sizing and provider lifetime limits before E2B control access.

use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use sandbox_interface::{
    BackendCreateSandboxRequest, Error, OperationId, ResourceAmount, ResourceOwner, ResourceUnit,
    SandboxConsumer, SandboxId, SandboxLifecycle, SandboxLifetime, SandboxResources,
    SandboxSizeSelection, SandboxState,
};
use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use crate::{
    ControlSandbox, ControlSandboxAccess, ControlSandboxState, E2bAdapterConfig, E2bControlApiMock,
    E2bProfile, E2bSandboxBackend,
};

#[tokio::test]
async fn six_hour_lifetime_is_rejected_before_create_and_recovery_access() {
    let (backend, calls) = backend();
    let mut request = request();
    request.lifetime = SandboxLifetime::OneShot {
        max_lifetime: Duration::from_secs(3600),
    };
    let accepted = backend.create_sandbox(request.clone()).await.unwrap();
    assert_eq!(accepted.state, SandboxState::Ready);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    request.lifetime = SandboxLifetime::OneShot {
        max_lifetime: Duration::from_secs(21_600),
    };

    for result in [
        backend.create_sandbox(request.clone()).await.map(Some),
        backend.recover_sandbox_create(request).await,
    ] {
        assert!(
            matches!(
                result,
                Err(Error::InvalidSeconds {
                    field: "max_lifetime",
                    minimum: 1,
                    maximum: 3600,
                })
            ),
            "expected E2B lifetime refusal, received {result:?}"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn explicit_sizes_are_rejected_before_create_and_recovery_access() {
    let (backend, calls) = backend();
    let sizes = [
        SandboxSizeSelection::ProviderSize("large".to_owned()),
        SandboxSizeSelection::Resources(SandboxResources {
            vcpus: 8,
            memory: ResourceAmount {
                amount: 16,
                unit: ResourceUnit::Gigabytes,
            },
            disk: ResourceAmount {
                amount: 111,
                unit: ResourceUnit::Gigabytes,
            },
        }),
    ];

    for selection in sizes {
        let mut request = request();
        request.size = Some(selection.clone());
        for result in [
            backend.create_sandbox(request.clone()).await.map(Some),
            backend.recover_sandbox_create(request).await,
        ] {
            assert!(
                matches!(result, Err(Error::UnsupportedSizeSelection {
                selection: ref rejected,
            }) if rejected == &selection),
                "expected exact size refusal, received {result:?}"
            );
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn managed_lifetime_above_the_e2b_limit_stays_unknown() {
    let (backend, calls) = backend();

    let managed = backend
        .list_managed_sandboxes("deployment".to_owned())
        .await
        .unwrap();

    assert_eq!(managed.len(), 1);
    assert_eq!(managed[0].lifetime, None);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

fn backend() -> (E2bSandboxBackend, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let control = Unimock::new((
        E2bControlApiMock::create_sandbox
            .each_call(matching!(_))
            .answers_arc({
                let calls = calls.clone();
                Arc::new(move |_, _| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(ControlSandboxAccess {
                        sandbox_id: "created".to_owned(),
                        domain: "e2b.app".to_owned(),
                        envd_access_token: "envd".to_owned(),
                        traffic_access_token: Some("traffic".to_owned()),
                    })
                })
            })
            .at_least_times(0),
        E2bControlApiMock::list_sandboxes
            .each_call(matching!(_))
            .answers_arc({
                let calls = calls.clone();
                Arc::new(move |_, _| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(vec![ControlSandbox {
                        sandbox_id: "created".to_owned(),
                        state: ControlSandboxState::Running,
                        metadata: BTreeMap::from([
                            ("sandbox_lifetime".to_owned(), "one_shot".to_owned()),
                            (
                                "sandbox_one_shot_max_lifetime_seconds".to_owned(),
                                "3601".to_owned(),
                            ),
                        ]),
                    }])
                })
            })
            .at_least_times(0),
    ))
    .no_verify_in_drop();
    let config = E2bAdapterConfig::new(
        "e2b",
        "https://api.e2b.app",
        "key",
        HashMap::from([(
            "general".to_owned(),
            E2bProfile {
                template: "base".to_owned(),
                allow_public_egress: true,
                denied_destinations: vec!["203.0.113.10/32".to_owned()],
            },
        )]),
        600,
    )
    .unwrap();
    (
        E2bSandboxBackend::with_transports(config, Arc::new(control), Arc::new(Unimock::new(()))),
        calls,
    )
}

fn request() -> BackendCreateSandboxRequest {
    BackendCreateSandboxRequest {
        sandbox_id: SandboxId::new(),
        operation_id: OperationId::new(),
        owner: ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7()),
        consumer: SandboxConsumer::Runtime,
        lifetime: SandboxLifetime::IdleAutoPause,
        size: None,
        deployment_id: "deployment".to_owned(),
        profile: "general".to_owned(),
        network: Default::default(),
        snapshot_provider_ref: None,
    }
}
