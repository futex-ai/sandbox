//! Image conformance recovery and cleanup regressions.

use crate::{
    BackendPreparedImage, BackendSandbox, BackendSnapshot, BackendSnapshotCreateOutcome,
    BackendSnapshotInventory, BackendSnapshotRecovery, Error, ProviderRef, RetainedSandboxRef,
    SandboxBackendMock, SandboxState, SnapshotState,
};
use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use super::exercise;

#[tokio::test]
async fn in_progress_snapshot_recovers_and_optional_diagnostics_cleanup() {
    exercise(
        &backend(BackendSnapshotCreateOutcome::InProgress, None),
        "test",
        Uuid::now_v7(),
    )
    .await
    .expect("in-progress image snapshot should recover");
}

#[tokio::test]
async fn ambiguous_snapshot_recovers_without_replaying_preparation() {
    exercise(
        &backend(BackendSnapshotCreateOutcome::DeliveryAmbiguous, None),
        "test",
        Uuid::now_v7(),
    )
    .await
    .expect("ambiguous image snapshot should recover");
}

#[tokio::test]
async fn omitted_retained_source_diagnostic_is_valid_and_cleanup_still_runs() {
    exercise(
        &backend(BackendSnapshotCreateOutcome::DeliveryAmbiguous, None),
        "test",
        Uuid::now_v7(),
    )
    .await
    .expect("the caller-known source should not require duplicate diagnostics");
}

#[tokio::test]
async fn preparation_transport_failure_cleans_the_created_source() {
    let backend = Unimock::new((
        SandboxBackendMock::create_sandbox
            .next_call(matching!(_))
            .returns(Ok(sandbox("prepared-source"))),
        SandboxBackendMock::prepare_image
            .next_call(matching!(_))
            .returns(Err(Error::BackendUnavailable {
                backend_id: "test".to_owned(),
            })),
        SandboxBackendMock::destroy_sandbox
            .next_call(matching!(_))
            .returns(Ok(())),
    ));

    let result = exercise(&backend, "test", Uuid::now_v7()).await;

    assert!(matches!(result, Err(Error::BackendUnavailable { .. })));
}

#[tokio::test]
async fn invalid_prepared_image_cleans_the_created_source() {
    let backend = Unimock::new((
        SandboxBackendMock::create_sandbox
            .next_call(matching!(_))
            .returns(Ok(sandbox("prepared-source"))),
        SandboxBackendMock::prepare_image
            .next_call(matching!(_))
            .returns(Ok(BackendPreparedImage {
                source_provider_ref: ProviderRef::new("different-source"),
                size_bytes: 4096,
            })),
        SandboxBackendMock::destroy_sandbox
            .next_call(matching!(_))
            .returns(Ok(())),
    ));

    let result = exercise(&backend, "test", Uuid::now_v7()).await;

    assert!(matches!(result, Err(Error::Internal(_))));
}

#[tokio::test]
async fn snapshot_inventory_failure_cleans_the_created_source() {
    let backend = Unimock::new((
        SandboxBackendMock::create_sandbox
            .next_call(matching!(_))
            .returns(Ok(sandbox("prepared-source"))),
        SandboxBackendMock::prepare_image
            .next_call(matching!(_))
            .returns(Ok(BackendPreparedImage {
                source_provider_ref: ProviderRef::new("prepared-source"),
                size_bytes: 4096,
            })),
        SandboxBackendMock::snapshot_inventory
            .next_call(matching!(_, _))
            .returns(Err(Error::BackendUnavailable {
                backend_id: "test".to_owned(),
            })),
        SandboxBackendMock::destroy_sandbox
            .next_call(matching!(_))
            .returns(Ok(())),
    ));

    let result = exercise(&backend, "test", Uuid::now_v7()).await;

    assert!(matches!(result, Err(Error::BackendUnavailable { .. })));
}

fn backend(
    snapshot_outcome: BackendSnapshotCreateOutcome,
    retained_sandbox: Option<RetainedSandboxRef>,
) -> Unimock {
    Unimock::new((
        SandboxBackendMock::create_sandbox
            .next_call(matching!(_))
            .returns(Ok(sandbox("prepared-source"))),
        SandboxBackendMock::prepare_image
            .next_call(matching!(_))
            .returns(Ok(BackendPreparedImage {
                source_provider_ref: ProviderRef::new("prepared-source"),
                size_bytes: 4096,
            })),
        SandboxBackendMock::snapshot_inventory
            .next_call(matching!(_, _))
            .returns(Ok(BackendSnapshotInventory::default())),
        SandboxBackendMock::create_snapshot
            .next_call(matching!(_))
            .returns(Ok(snapshot_outcome)),
        SandboxBackendMock::recover_snapshot_create
            .each_call(matching!(_))
            .answers(&|_, _| Ok(BackendSnapshotRecovery::Recovered(snapshot())))
            .at_least_times(0),
        SandboxBackendMock::delete_snapshot
            .next_call(matching!(_))
            .returns(Ok(())),
        SandboxBackendMock::destroy_sandbox
            .next_call(matching!(_))
            .returns(Ok(())),
        SandboxBackendMock::create_sandbox
            .next_call(matching!(_))
            .returns(Ok(sandbox("failed-source"))),
        SandboxBackendMock::prepare_image
            .next_call(matching!(_))
            .returns(Err(Error::ImageSetupFailed {
                command: None,
                retained_sandbox,
            })),
        SandboxBackendMock::destroy_sandbox
            .next_call(matching!(_))
            .returns(Ok(())),
    ))
}

fn sandbox(provider_ref: &str) -> BackendSandbox {
    BackendSandbox {
        provider_ref: ProviderRef::new(provider_ref),
        state: SandboxState::Ready,
    }
}

fn snapshot() -> BackendSnapshot {
    BackendSnapshot {
        provider_ref: ProviderRef::new("prepared-image"),
        state: SnapshotState::Ready,
    }
}
