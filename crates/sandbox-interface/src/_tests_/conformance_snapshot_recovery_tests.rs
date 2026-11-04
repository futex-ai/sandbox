//! Image conformance snapshot-recovery regressions.

use std::time::Duration;

use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use crate::{
    BackendCreateSnapshotRequest, BackendPreparedImage, BackendSandbox, BackendSnapshot,
    BackendSnapshotCreateOutcome, BackendSnapshotInventory, BackendSnapshotRecovery, Error,
    OperationId, ProviderRef, RetainedSandboxRef, SandboxImagePreparationMock,
    SandboxLifecycleMock, SandboxSnapshotsMock, SandboxState, SnapshotId, SnapshotState,
};

use super::exercise;
use crate::conformance_resources::{ConformanceResources, RecoverySleeperMock};

#[tokio::test]
async fn in_progress_snapshot_recovers_and_optional_diagnostics_cleanup() {
    let backend = backend(BackendSnapshotCreateOutcome::InProgress, None);
    exercise(&backend, &backend, &backend, "test", Uuid::now_v7())
        .await
        .expect("in-progress image snapshot should recover");
}

#[tokio::test]
async fn ambiguous_snapshot_recovers_without_replaying_preparation() {
    let backend = backend(BackendSnapshotCreateOutcome::DeliveryAmbiguous, None);
    exercise(&backend, &backend, &backend, "test", Uuid::now_v7())
        .await
        .expect("ambiguous image snapshot should recover");
}

#[tokio::test]
async fn omitted_retained_source_diagnostic_is_valid_and_cleanup_still_runs() {
    let backend = backend(BackendSnapshotCreateOutcome::DeliveryAmbiguous, None);
    exercise(&backend, &backend, &backend, "test", Uuid::now_v7())
        .await
        .expect("the caller-known source should not require duplicate diagnostics");
}

#[tokio::test]
async fn in_progress_recovery_waits_one_second_before_each_retry() {
    let backend = Unimock::new((
        SandboxSnapshotsMock::create_snapshot
            .next_call(matching!(_))
            .returns(Ok(BackendSnapshotCreateOutcome::InProgress)),
        SandboxSnapshotsMock::recover_snapshot_create
            .next_call(matching!(_))
            .returns(Ok(BackendSnapshotRecovery::InProgress)),
        SandboxSnapshotsMock::recover_snapshot_create
            .next_call(matching!(_))
            .returns(Ok(BackendSnapshotRecovery::InProgress)),
        SandboxSnapshotsMock::recover_snapshot_create
            .next_call(matching!(_))
            .returns(Ok(BackendSnapshotRecovery::Recovered(snapshot()))),
    ));
    let sleeper = Unimock::new((
        RecoverySleeperMock::sleep
            .next_call(matching!(_))
            .answers(&|_, duration| assert_eq!(duration, Duration::from_secs(1))),
        RecoverySleeperMock::sleep
            .next_call(matching!(_))
            .answers(&|_, duration| assert_eq!(duration, Duration::from_secs(1))),
    ));

    let mut resources = ConformanceResources::new(&sleeper);
    let recovered = resources
        .create_snapshot(&backend, snapshot_request())
        .await
        .expect("snapshot should recover after paced retries");

    assert_eq!(recovered, snapshot());
}

fn backend(
    snapshot_outcome: BackendSnapshotCreateOutcome,
    retained_sandbox: Option<RetainedSandboxRef>,
) -> Unimock {
    Unimock::new((
        SandboxLifecycleMock::create_sandbox
            .next_call(matching!(_))
            .returns(Ok(sandbox("prepared-source"))),
        SandboxLifecycleMock::recover_sandbox_create
            .next_call(matching!(_))
            .returns(Ok(Some(sandbox("prepared-source")))),
        SandboxImagePreparationMock::prepare_image
            .next_call(matching!(_))
            .returns(Ok(BackendPreparedImage {
                source_provider_ref: ProviderRef::new("prepared-source"),
                size_bytes: 4096,
            })),
        SandboxSnapshotsMock::snapshot_inventory
            .next_call(matching!(_, _))
            .returns(Ok(BackendSnapshotInventory::default())),
        SandboxSnapshotsMock::create_snapshot
            .next_call(matching!(_))
            .returns(Ok(snapshot_outcome)),
        SandboxSnapshotsMock::recover_snapshot_create
            .each_call(matching!(_))
            .answers(&|_, _| Ok(BackendSnapshotRecovery::Recovered(snapshot())))
            .at_least_times(0),
        SandboxLifecycleMock::create_sandbox
            .next_call(matching!(_))
            .returns(Ok(sandbox("failed-source"))),
        SandboxLifecycleMock::recover_sandbox_create
            .next_call(matching!(_))
            .returns(Ok(Some(sandbox("failed-source")))),
        SandboxImagePreparationMock::prepare_image
            .next_call(matching!(_))
            .returns(Err(Error::ImageSetupFailed {
                command: None,
                retained_sandbox,
            })),
        SandboxSnapshotsMock::delete_snapshot
            .next_call(matching!(_))
            .returns(Ok(())),
        SandboxLifecycleMock::destroy_sandbox
            .next_call(matching!(_))
            .returns(Ok(())),
        SandboxLifecycleMock::destroy_sandbox
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

fn snapshot_request() -> BackendCreateSnapshotRequest {
    BackendCreateSnapshotRequest {
        snapshot_id: SnapshotId::new(),
        operation_id: OperationId::new(),
        source_provider_ref: ProviderRef::new("prepared-source"),
        correlation_name: "paced-recovery".to_owned(),
        before: BackendSnapshotInventory::default(),
    }
}
