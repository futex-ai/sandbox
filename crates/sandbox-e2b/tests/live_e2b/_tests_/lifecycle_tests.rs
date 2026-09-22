//! Live lifecycle recovery regressions using mocked provider boundaries.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use sandbox_e2b::{
    ControlSandbox, ControlSandboxAccess, ControlSandboxState, ControlSnapshot, E2bAdapterError,
    E2bControlApiMock, E2bSandboxBackend, ProcessTransportMock,
};
use sandbox_interface::{
    BackendCreateSnapshotRequest, BackendSnapshotInventory, Error as SandboxError, OperationId,
    ProviderRef, ResourceOwner, SnapshotId,
};
use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use super::{restore, run, snapshot};
use crate::support::{LiveResources, live_config};

#[tokio::test]
async fn ambiguous_snapshot_creation_recovers_the_cleanup_handle() {
    let control = Unimock::new((
        E2bControlApiMock::list_snapshots
            .next_call(matching!("source", _))
            .returns(Ok(Vec::new())),
        E2bControlApiMock::create_snapshot
            .next_call(matching!("source", _))
            .returns(Err(E2bAdapterError::DeliveryAmbiguous)),
        E2bControlApiMock::list_snapshots
            .next_call(matching!("source", _))
            .returns(Ok(vec![ControlSnapshot {
                snapshot_id: "recovered-snapshot".to_owned(),
            }])),
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("source"))
            .returns(Ok(access())),
    ));
    let backend = E2bSandboxBackend::with_transports(
        live_config("api-key".to_owned(), "base".to_owned()),
        Arc::new(control),
        Arc::new(Unimock::new(())),
    );

    let mut resources = LiveResources::default();
    let recovered = snapshot(&backend, ProviderRef::new("source"), &mut resources)
        .await
        .expect("ambiguous snapshot creation should recover its provider handle");

    assert_eq!(recovered.as_str(), "recovered-snapshot");
    assert_eq!(resources.snapshot.as_ref(), Some(&recovered));
    assert!(resources.snapshot_request.is_some());
}

#[tokio::test]
async fn cleanup_recovers_and_deletes_a_pending_snapshot_before_its_source() {
    let sequence = Arc::new(AtomicUsize::new(0));
    let control = Unimock::new((
        E2bControlApiMock::list_snapshots
            .next_call(matching!("source", _))
            .answers_arc({
                let sequence = sequence.clone();
                Arc::new(move |_, _, _| {
                    assert_eq!(sequence.fetch_add(1, Ordering::Relaxed), 0);
                    Ok(vec![ControlSnapshot {
                        snapshot_id: "recovered-snapshot".to_owned(),
                    }])
                })
            }),
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("source"))
            .answers_arc({
                let sequence = sequence.clone();
                Arc::new(move |_, _| {
                    assert_eq!(sequence.fetch_add(1, Ordering::Relaxed), 1);
                    Ok(access())
                })
            }),
        E2bControlApiMock::delete_snapshot
            .next_call(matching!("recovered-snapshot"))
            .answers_arc({
                let sequence = sequence.clone();
                Arc::new(move |_, _| {
                    assert_eq!(sequence.fetch_add(1, Ordering::Relaxed), 2);
                    Ok(())
                })
            }),
        E2bControlApiMock::kill_sandbox
            .next_call(matching!("source"))
            .answers_arc({
                let sequence = sequence.clone();
                Arc::new(move |_, _| {
                    assert_eq!(sequence.fetch_add(1, Ordering::Relaxed), 3);
                    Ok(())
                })
            }),
    ));
    let backend = E2bSandboxBackend::with_transports(
        live_config("api-key".to_owned(), "base".to_owned()),
        Arc::new(control),
        Arc::new(Unimock::new(())),
    );
    let resources = LiveResources {
        sandboxes: vec![ProviderRef::new("source")],
        snapshot_request: Some(snapshot_request()),
        ..LiveResources::default()
    };

    resources
        .cleanup(&backend)
        .await
        .expect("pending snapshot cleanup should recover before source deletion");

    assert_eq!(sequence.load(Ordering::Relaxed), 4);
}

#[tokio::test]
async fn failed_live_source_create_is_recovered_and_destroyed_during_cleanup() {
    let control = Unimock::new((
        E2bControlApiMock::list_sandboxes
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
        E2bControlApiMock::create_sandbox
            .next_call(matching!(_))
            .returns(Err(E2bAdapterError::DeliveryAmbiguous)),
        E2bControlApiMock::list_sandboxes
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
        E2bControlApiMock::list_sandboxes
            .next_call(matching!(_))
            .returns(Err(E2bAdapterError::Unavailable)),
        E2bControlApiMock::list_sandboxes
            .next_call(matching!(_))
            .returns(Ok(vec![ControlSandbox {
                sandbox_id: "recovered-source".to_owned(),
                state: ControlSandboxState::Running,
                metadata: Default::default(),
            }])),
        E2bControlApiMock::kill_sandbox
            .next_call(matching!("recovered-source"))
            .returns(Ok(())),
    ));
    let config = live_config("api-key".to_owned(), "base".to_owned());
    let backend = E2bSandboxBackend::with_transports(
        config.clone(),
        Arc::new(control),
        Arc::new(Unimock::new(())),
    );
    let mut resources = LiveResources::default();

    let result = run(
        &backend,
        &config,
        ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7()),
        &mut resources,
    )
    .await;
    assert!(result.is_err(), "uncertain creation should remain pending");

    resources
        .cleanup(&backend)
        .await
        .expect("cleanup should recover and destroy the pending sandbox");
}

#[tokio::test]
async fn restored_sandbox_is_tracked_before_terminal_cleanup_can_fail() {
    let control = Unimock::new((
        E2bControlApiMock::list_sandboxes
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
        E2bControlApiMock::create_sandbox
            .next_call(matching!(_))
            .returns(Ok(access_for("restore-source"))),
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("restore-source"))
            .returns(Ok(access_for("restore-source"))),
        E2bControlApiMock::kill_sandbox
            .next_call(matching!("restore-source"))
            .returns(Ok(())),
    ));
    let processes = Unimock::new(
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Err(SandboxError::BackendUnavailable {
                backend_id: "e2b-live".to_owned(),
            })),
    );
    let backend = E2bSandboxBackend::with_transports(
        live_config("api-key".to_owned(), "base".to_owned()),
        Arc::new(control),
        Arc::new(processes),
    );
    let mut resources = LiveResources::default();

    let result = restore(
        &backend,
        ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7()),
        ProviderRef::new("snapshot"),
        &mut resources,
    )
    .await;
    assert!(result.is_err(), "restored-terminal cleanup should fail");

    resources
        .cleanup(&backend)
        .await
        .expect("cleanup should destroy the already-created restore");
}

fn snapshot_request() -> BackendCreateSnapshotRequest {
    BackendCreateSnapshotRequest {
        snapshot_id: SnapshotId::new(),
        operation_id: OperationId::new(),
        source_provider_ref: ProviderRef::new("source"),
        correlation_name: "live-pending".to_owned(),
        before: BackendSnapshotInventory::default(),
    }
}

fn access() -> ControlSandboxAccess {
    access_for("source")
}

fn access_for(sandbox_id: &str) -> ControlSandboxAccess {
    ControlSandboxAccess {
        sandbox_id: sandbox_id.to_owned(),
        domain: "e2b.app".to_owned(),
        envd_access_token: "envd-token".to_owned(),
        traffic_access_token: Some("traffic-token".to_owned()),
    }
}
