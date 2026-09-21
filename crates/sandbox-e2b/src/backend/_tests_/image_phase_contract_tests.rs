//! Durable caller-driven image phase regressions.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendPrepareImageRequest,
    BackendSnapshotInventory, BackendSnapshotRecovery, Error, OperationId, ProviderRef,
    ResourceOwner, SandboxBackend, SandboxId, SandboxNetworkPolicy, SnapshotId,
};
use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bAdapterError, E2bControlApiMock, E2bProfile,
    ProcessRunOutput, ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn preparation_uses_only_the_caller_persisted_source() {
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("persisted-source"))
            .returns(Ok(access("persisted-source"))),
    );
    let processes = Unimock::new((
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(success())),
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(success())),
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(size_output(4096))),
    ));
    let prepared = backend(control, processes)
        .prepare_image(prepare_request())
        .await
        .expect("preparation should not allocate or snapshot provider resources");

    assert_eq!(prepared.source_provider_ref.as_str(), "persisted-source");
    assert_eq!(prepared.size_bytes, 4096);
}

#[tokio::test]
async fn preparation_failure_leaves_the_persisted_source_caller_owned() {
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("persisted-source"))
            .returns(Err(E2bAdapterError::Unavailable)),
    );

    let result = backend(control, Unimock::new(()))
        .prepare_image(prepare_request())
        .await;

    assert!(matches!(result, Err(Error::BackendUnavailable { .. })));
}

#[tokio::test]
async fn empty_snapshot_recovery_does_not_rebuild_or_redispatch() {
    let control = Unimock::new((
        E2bControlApiMock::list_snapshots
            .next_call(matching!("persisted-source", "image-operation"))
            .returns(Ok(Vec::new())),
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("persisted-source"))
            .returns(Ok(access("persisted-source"))),
    ));
    let recovery = backend(control, Unimock::new(()))
        .recover_snapshot_create(snapshot_request())
        .await
        .expect("empty eventual-consistency inventory remains in progress");

    assert_eq!(recovery, BackendSnapshotRecovery::InProgress);
}

#[tokio::test]
async fn empty_source_recovery_does_not_allocate_a_second_sandbox() {
    let control = Unimock::new(
        E2bControlApiMock::list_sandboxes
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
    );
    let recovered = backend(control, Unimock::new(()))
        .recover_sandbox_create(source_request())
        .await
        .expect("empty eventual-consistency inventory is not a new create request");

    assert!(recovered.is_none());
}

fn prepare_request() -> BackendPrepareImageRequest {
    BackendPrepareImageRequest {
        sandbox_id: SandboxId::new(),
        source_provider_ref: ProviderRef::new("persisted-source"),
        owner: ResourceOwner::platform(Uuid::now_v7()),
        input_files: Vec::new(),
        setup_script: "true".to_owned(),
        verify_commands: Vec::new(),
    }
}

fn source_request() -> BackendCreateSandboxRequest {
    BackendCreateSandboxRequest {
        sandbox_id: SandboxId::new(),
        operation_id: OperationId::new(),
        owner: ResourceOwner::platform(Uuid::now_v7()),
        consumer: sandbox_interface::SandboxConsumer::Runtime,
        deployment_id: "deployment".to_owned(),
        profile: "general".to_owned(),
        network: SandboxNetworkPolicy::Open,
        snapshot_provider_ref: None,
    }
}

fn snapshot_request() -> BackendCreateSnapshotRequest {
    BackendCreateSnapshotRequest {
        snapshot_id: SnapshotId::new(),
        operation_id: OperationId::new(),
        source_provider_ref: ProviderRef::new("persisted-source"),
        correlation_name: "image-operation".to_owned(),
        before: BackendSnapshotInventory::default(),
    }
}

fn backend(control: Unimock, processes: Unimock) -> E2bSandboxBackend {
    E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(processes))
}

fn config() -> E2bAdapterConfig {
    E2bAdapterConfig::new(
        "configured-e2b",
        "https://api.e2b.app",
        "api-key",
        HashMap::from([(
            "general".to_owned(),
            E2bProfile {
                template: "base".to_owned(),
                allow_public_egress: false,
                denied_destinations: vec!["203.0.113.10/32".to_owned()],
            },
        )]),
        600,
    )
    .expect("valid adapter config")
}

fn access(sandbox_id: &str) -> ControlSandboxAccess {
    ControlSandboxAccess {
        sandbox_id: sandbox_id.to_owned(),
        domain: "e2b.app".to_owned(),
        envd_access_token: "call-local-token".to_owned(),
        traffic_access_token: "traffic-token".to_owned(),
    }
}

fn success() -> ProcessRunOutput {
    ProcessRunOutput {
        bytes: Vec::new(),
        exit_code: Some(0),
        exited: true,
        output_truncated: false,
    }
}

fn size_output(size: u64) -> ProcessRunOutput {
    ProcessRunOutput {
        bytes: format!("__SANDBOX_IMAGE_SIZE__={size}\n").into_bytes(),
        ..success()
    }
}
