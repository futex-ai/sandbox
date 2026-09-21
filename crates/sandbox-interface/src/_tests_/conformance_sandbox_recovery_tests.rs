//! Image-source sandbox recovery pacing regressions.

use std::time::Duration;

use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use crate::{
    BackendCreateSandboxRequest, BackendSandbox, Error, OperationId, ProviderRef, ResourceOwner,
    SandboxBackendMock, SandboxId, SandboxNetworkPolicy, SandboxState,
};

use super::{RecoverySleeperMock, create_or_recover_sandbox_with_sleeper};

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

    let recovered = create_or_recover_sandbox_with_sleeper(&backend, request, &sleeper)
        .await
        .expect("sandbox should recover after paced retries");

    assert_eq!(recovered, sandbox());
}

fn sandbox_request() -> BackendCreateSandboxRequest {
    BackendCreateSandboxRequest {
        sandbox_id: SandboxId::new(),
        operation_id: OperationId::new(),
        owner: ResourceOwner::platform(Uuid::now_v7()),
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
