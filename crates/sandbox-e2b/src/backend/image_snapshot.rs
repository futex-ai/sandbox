//! Bounded E2B image-snapshot creation and ambiguous-delivery recovery.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use sandbox_interface::{
    BackendCreateSnapshotRequest, BackendSnapshotCreateOutcome, BackendSnapshotRecovery, Error,
    OperationId, ProviderRef, Result, SnapshotId,
};

use super::{configured::E2bSandboxBackend, snapshots};

const RECOVERY_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RECOVERY_WAITS: usize = 60;

#[unimock::unimock(api = SnapshotRecoverySleeperMock)]
#[async_trait]
pub(super) trait SnapshotRecoverySleeper: Send + Sync {
    async fn sleep(&self, duration: Duration);
}

pub(super) type DynSnapshotRecoverySleeper = Arc<dyn SnapshotRecoverySleeper>;

pub(super) struct TokioSnapshotRecoverySleeper;

#[async_trait]
impl SnapshotRecoverySleeper for TokioSnapshotRecoverySleeper {
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

pub(super) async fn realize(
    backend: &E2bSandboxBackend,
    snapshot_id: SnapshotId,
    operation_id: OperationId,
    correlation_name: String,
    source_provider_ref: ProviderRef,
) -> Result<ProviderRef> {
    let before = snapshots::inventory(
        backend,
        source_provider_ref.clone(),
        correlation_name.clone(),
    )
    .await?;
    match before.snapshots.as_slice() {
        [existing] => return Ok(existing.clone()),
        [] => {}
        _ => return Err(Error::SnapshotReconciliationRequired),
    }
    let create_request = BackendCreateSnapshotRequest {
        snapshot_id,
        operation_id,
        source_provider_ref,
        correlation_name,
        before,
    };
    match snapshots::create(backend, create_request.clone()).await? {
        BackendSnapshotCreateOutcome::Created(snapshot) => Ok(snapshot.provider_ref),
        BackendSnapshotCreateOutcome::InProgress
        | BackendSnapshotCreateOutcome::DeliveryAmbiguous => {
            recover_ambiguous_snapshot(backend, create_request).await
        }
    }
}

async fn recover_ambiguous_snapshot(
    backend: &E2bSandboxBackend,
    request: BackendCreateSnapshotRequest,
) -> Result<ProviderRef> {
    let mut waits_remaining = MAX_RECOVERY_WAITS;
    loop {
        match snapshots::recover(backend, request.clone()).await? {
            BackendSnapshotRecovery::Recovered(snapshot) => return Ok(snapshot.provider_ref),
            BackendSnapshotRecovery::ReconciliationRequired => {
                return Err(Error::SnapshotReconciliationRequired);
            }
            BackendSnapshotRecovery::InProgress if waits_remaining == 0 => {
                return Err(Error::SnapshotReconciliationRequired);
            }
            BackendSnapshotRecovery::InProgress => {
                waits_remaining -= 1;
                backend
                    .snapshot_recovery_sleeper
                    .sleep(RECOVERY_INTERVAL)
                    .await;
            }
        }
    }
}
