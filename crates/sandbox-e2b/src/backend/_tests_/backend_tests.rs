//! Mocked E2B backend behavior and reconciliation tests.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendSnapshotCreateOutcome,
    BackendSnapshotInventory, BackendSnapshotRecovery, Error, OperationId, ProviderRef,
    ResourceOwner, SandboxBackend, SandboxId, SandboxState, SnapshotId,
};
use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use crate::{
    ControlSandbox, ControlSandboxAccess, ControlSandboxState, ControlSnapshot, E2bAdapterConfig,
    E2bAdapterError, E2bControlApiMock, E2bProfile,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn ambiguous_sandbox_create_recovers_by_exact_metadata() {
    let request = sandbox_request(None);
    let expected_metadata = std::collections::BTreeMap::from([
        (
            "sandbox_agent_id".to_owned(),
            request
                .owner
                .agent_id
                .expect("test owner should be an agent")
                .to_string(),
        ),
        ("sandbox_deployment_id".to_owned(), "deployment".to_owned()),
        (
            "sandbox_operation_id".to_owned(),
            request.operation_id.to_string(),
        ),
        ("sandbox_consumer".to_owned(), "runtime".to_owned()),
        ("sandbox_owner_kind".to_owned(), "agent".to_owned()),
        (
            "sandbox_sandbox_id".to_owned(),
            request.sandbox_id.to_string(),
        ),
        (
            "sandbox_workspace_id".to_owned(),
            request.owner.workspace_id.to_string(),
        ),
    ]);
    let created_metadata = expected_metadata.clone();
    let control = Unimock::new((
        E2bControlApiMock::create_sandbox
            .next_call(matching!(_))
            .answers_arc(Arc::new(move |_, create| {
                assert_eq!(create.metadata, created_metadata);
                assert_eq!(create.template_id, "base");
                Err(E2bAdapterError::DeliveryAmbiguous)
            })),
        E2bControlApiMock::list_sandboxes
            .next_call(matching!(_))
            .returns(Ok(vec![ControlSandbox {
                sandbox_id: "provider-sandbox".to_owned(),
                state: ControlSandboxState::Running,
                metadata: Default::default(),
            }])),
    ));
    let backend = backend(control, Unimock::new(()));

    let sandbox = backend
        .create_sandbox(request)
        .await
        .expect("one exact candidate should recover");

    assert_eq!(sandbox.provider_ref.as_str(), "provider-sandbox");
    assert_eq!(sandbox.state, SandboxState::Ready);
}

#[tokio::test]
async fn snapshot_create_reconnects_source_and_recovery_diffs_inventory() {
    let source = ProviderRef::new("source");
    let access = access("source");
    let control = Unimock::new((
        E2bControlApiMock::create_snapshot
            .next_call(matching!("source", "sandbox-operation"))
            .returns(Ok(ControlSnapshot {
                snapshot_id: "snapshot-created".to_owned(),
            })),
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("source"))
            .returns(Ok(access.clone())),
        E2bControlApiMock::list_snapshots
            .next_call(matching!("source", "sandbox-operation"))
            .returns(Ok(vec![
                ControlSnapshot {
                    snapshot_id: "snapshot-before".to_owned(),
                },
                ControlSnapshot {
                    snapshot_id: "snapshot-new".to_owned(),
                },
            ])),
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("source"))
            .returns(Ok(access)),
    ));
    let backend = backend(control, Unimock::new(()));
    let mut request = snapshot_request(source);

    let created = backend
        .create_snapshot(request.clone())
        .await
        .expect("snapshot create");
    request.before.snapshots = vec![ProviderRef::new("snapshot-before")];
    let recovered = backend
        .recover_snapshot_create(request)
        .await
        .expect("snapshot recovery");

    assert!(matches!(
        created,
        BackendSnapshotCreateOutcome::Created(snapshot)
            if snapshot.provider_ref.as_str() == "snapshot-created"
    ));
    assert!(matches!(
        recovered,
        BackendSnapshotRecovery::Recovered(snapshot)
            if snapshot.provider_ref.as_str() == "snapshot-new"
    ));
}

#[tokio::test]
async fn snapshot_recovery_keeps_the_source_paused_without_one_candidate() {
    let control = Unimock::new((
        E2bControlApiMock::list_snapshots
            .next_call(matching!(_, _))
            .returns(Ok(Vec::new())),
        E2bControlApiMock::list_snapshots
            .next_call(matching!(_, _))
            .returns(Ok(vec![
                ControlSnapshot {
                    snapshot_id: "one".to_owned(),
                },
                ControlSnapshot {
                    snapshot_id: "two".to_owned(),
                },
            ])),
    ));
    let backend = backend(control, Unimock::new(()));

    let zero = backend
        .recover_snapshot_create(snapshot_request(ProviderRef::new("source")))
        .await
        .expect("zero candidate recovery");
    let multiple = backend
        .recover_snapshot_create(snapshot_request(ProviderRef::new("source")))
        .await
        .expect("multiple candidate recovery");

    assert_eq!(zero, BackendSnapshotRecovery::InProgress);
    assert_eq!(multiple, BackendSnapshotRecovery::ReconciliationRequired);
}

#[tokio::test]
async fn existing_sandbox_operations_reject_mismatched_provider_identity() {
    let inspect_backend = backend(
        Unimock::new(
            E2bControlApiMock::get_sandbox
                .next_call(matching!("expected"))
                .returns(Ok(ControlSandbox {
                    sandbox_id: "different".to_owned(),
                    state: ControlSandboxState::Running,
                    metadata: Default::default(),
                })),
        ),
        Unimock::new(()),
    );
    let resume_backend = backend(
        Unimock::new(
            E2bControlApiMock::connect_sandbox
                .next_call(matching!("expected"))
                .returns(Ok(access("different"))),
        ),
        Unimock::new(()),
    );

    assert!(matches!(
        inspect_backend
            .inspect_sandbox(ProviderRef::new("expected"))
            .await,
        Err(Error::Internal(_))
    ));
    assert!(matches!(
        resume_backend
            .resume_sandbox(ProviderRef::new("expected"))
            .await,
        Err(Error::Internal(_))
    ));
}

#[tokio::test]
async fn envd_connection_rejects_mismatched_provider_identity() {
    let backend = backend(
        Unimock::new(
            E2bControlApiMock::connect_sandbox
                .next_call(matching!("expected"))
                .returns(Ok(access("different"))),
        ),
        Unimock::new(()),
    );

    assert!(matches!(
        super::mapping::connection(&backend, &ProviderRef::new("expected")).await,
        Err(Error::Internal(_))
    ));
}

#[tokio::test]
async fn envd_connection_uses_the_configured_domain() {
    let mut provider_access = access("expected");
    provider_access.domain = "untrusted.example".to_owned();
    let backend = backend(
        Unimock::new(
            E2bControlApiMock::connect_sandbox
                .next_call(matching!("expected"))
                .returns(Ok(provider_access)),
        ),
        Unimock::new(()),
    );

    let connection = super::mapping::connection(&backend, &ProviderRef::new("expected"))
        .await
        .expect("matching provider identity");

    assert_eq!(connection.sandbox_domain(), "e2b.app");
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

fn sandbox_request(snapshot_provider_ref: Option<ProviderRef>) -> BackendCreateSandboxRequest {
    BackendCreateSandboxRequest {
        network: sandbox_interface::SandboxNetworkPolicy::Open,
        sandbox_id: SandboxId::new(),
        operation_id: OperationId::new(),
        owner: ResourceOwner {
            workspace_id: Uuid::now_v7(),
            agent_id: Some(Uuid::now_v7()),
        },
        consumer: sandbox_interface::SandboxConsumer::Runtime,
        deployment_id: "deployment".to_owned(),
        profile: "general".to_owned(),
        snapshot_provider_ref,
    }
}

fn snapshot_request(source_provider_ref: ProviderRef) -> BackendCreateSnapshotRequest {
    BackendCreateSnapshotRequest {
        snapshot_id: SnapshotId::new(),
        operation_id: OperationId::new(),
        source_provider_ref,
        correlation_name: "sandbox-operation".to_owned(),
        before: BackendSnapshotInventory::default(),
    }
}

fn access(sandbox_id: &str) -> ControlSandboxAccess {
    ControlSandboxAccess {
        sandbox_id: sandbox_id.to_owned(),
        domain: "e2b.app".to_owned(),
        envd_access_token: "call-local-token".to_owned(),
        traffic_access_token: "traffic-token".to_owned(),
    }
}
