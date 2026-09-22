//! Image-source sandbox recovery pacing regressions.

use std::time::Duration;

use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use crate::{
    BackendCreateSandboxRequest, BackendSandbox, Error, OperationId, ProviderRef, ResourceOwner,
    SandboxBackendMock, SandboxConsumer, SandboxId, SandboxNetworkPolicy, SandboxState,
};

use crate::conformance_resources::{ConformanceResources, RecoverySleeperMock, finish};

#[tokio::test]
async fn uncertain_source_creation_uses_paced_recover_only_polling() {
    let request = sandbox_request();
    let backend = Unimock::new((
        SandboxBackendMock::create_sandbox
            .next_call(matching!(_))
            .returns(Err(Error::BackendUnavailable {
                backend_id: "test".to_owned(),
            })),
        SandboxBackendMock::recover_sandbox_create
            .next_call(matching!(_))
            .returns(Ok(None)),
        SandboxBackendMock::recover_sandbox_create
            .next_call(matching!(_))
            .returns(Ok(None)),
        SandboxBackendMock::recover_sandbox_create
            .next_call(matching!(_))
            .returns(Ok(Some(sandbox()))),
    ));
    let sleeper = Unimock::new((
        RecoverySleeperMock::sleep
            .next_call(matching!(_))
            .answers(&|_, duration| assert_eq!(duration, Duration::from_secs(1))),
        RecoverySleeperMock::sleep
            .next_call(matching!(_))
            .answers(&|_, duration| assert_eq!(duration, Duration::from_secs(1))),
    ));

    let mut resources = ConformanceResources::new(&backend, &sleeper);
    let recovered = resources
        .create_sandbox(request)
        .await
        .expect("sandbox should recover after paced retries");

    assert_eq!(recovered, sandbox());
}

#[tokio::test]
async fn cleanup_recovers_a_request_left_pending_by_a_recovery_error() {
    let backend = Unimock::new((
        SandboxBackendMock::create_sandbox
            .next_call(matching!(_))
            .returns(Err(Error::BackendUnavailable {
                backend_id: "create".to_owned(),
            })),
        SandboxBackendMock::recover_sandbox_create
            .next_call(matching!(_))
            .returns(Err(Error::BackendUnavailable {
                backend_id: "recover".to_owned(),
            })),
        SandboxBackendMock::recover_sandbox_create
            .next_call(matching!(_))
            .returns(Ok(Some(sandbox()))),
        SandboxBackendMock::destroy_sandbox
            .next_call(matching!(_))
            .answers(&|_, provider_ref| {
                assert_eq!(provider_ref, ProviderRef::new("recovered-source"));
                Ok(())
            }),
    ));
    let sleeper = Unimock::new(());
    let mut resources = ConformanceResources::new(&backend, &sleeper);

    let outcome = resources.create_sandbox(sandbox_request()).await;
    let cleanup = resources.cleanup().await;

    assert!(matches!(outcome, Err(Error::BackendUnavailable { .. })));
    cleanup.expect("final cleanup should recover and destroy the pending sandbox");
}

#[test]
fn operation_error_is_preserved_when_cleanup_also_fails() {
    let result = finish::<()>(
        Err(Error::UnknownProfile),
        Err(Error::BackendUnavailable {
            backend_id: "cleanup".to_owned(),
        }),
    );

    assert!(matches!(result, Err(Error::UnknownProfile)));
}

fn sandbox_request() -> BackendCreateSandboxRequest {
    BackendCreateSandboxRequest {
        sandbox_id: SandboxId::new(),
        operation_id: OperationId::new(),
        owner: ResourceOwner::platform(Uuid::now_v7()),
        consumer: SandboxConsumer::Runtime,
        lifetime: crate::SandboxLifetime::IdleAutoPause,
        deployment_id: "backend-conformance".to_owned(),
        profile: "test".to_owned(),
        network: SandboxNetworkPolicy::Open,
        snapshot_provider_ref: None,
    }
}

fn sandbox() -> BackendSandbox {
    BackendSandbox {
        provider_ref: ProviderRef::new("recovered-source"),
        state: SandboxState::Ready,
    }
}
