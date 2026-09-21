//! E2B one-to-many snapshot lifecycle and at-most-once recovery.

use std::collections::HashSet;

use sandbox_interface::{
    BackendCreateSnapshotRequest, BackendInspectSnapshotRequest, BackendSnapshot,
    BackendSnapshotCreateOutcome, BackendSnapshotInventory, BackendSnapshotRecovery, Error,
    ProviderRef, ResourceKind, Result,
};

use crate::error::Error as AdapterError;

use super::{configured::E2bSandboxBackend, mapping};

pub(super) async fn inventory(
    backend: &E2bSandboxBackend,
    source_provider_ref: ProviderRef,
    correlation_name: String,
) -> Result<BackendSnapshotInventory> {
    let snapshots = control_result(
        backend,
        backend
            .control
            .list_snapshots(source_provider_ref.as_str(), &correlation_name)
            .await,
    )?;
    Ok(BackendSnapshotInventory {
        snapshots: snapshots
            .into_iter()
            .map(|snapshot| ProviderRef::new(snapshot.snapshot_id))
            .collect(),
    })
}

pub(super) async fn create(
    backend: &E2bSandboxBackend,
    request: BackendCreateSnapshotRequest,
) -> Result<BackendSnapshotCreateOutcome> {
    match backend
        .control
        .create_snapshot(
            request.source_provider_ref.as_str(),
            &request.correlation_name,
        )
        .await
    {
        Ok(snapshot) => {
            reconnect_source(backend, &request.source_provider_ref).await;
            Ok(BackendSnapshotCreateOutcome::Created(
                mapping::ready_snapshot(snapshot.snapshot_id),
            ))
        }
        Err(AdapterError::DeliveryAmbiguous) => Ok(BackendSnapshotCreateOutcome::DeliveryAmbiguous),
        Err(error) => Err(map_control(backend, error)),
    }
}

pub(super) async fn recover(
    backend: &E2bSandboxBackend,
    request: BackendCreateSnapshotRequest,
) -> Result<BackendSnapshotRecovery> {
    let before = request
        .before
        .snapshots
        .iter()
        .map(ProviderRef::as_str)
        .collect::<HashSet<_>>();
    let mut candidates = control_result(
        backend,
        backend
            .control
            .list_snapshots(
                request.source_provider_ref.as_str(),
                &request.correlation_name,
            )
            .await,
    )?
    .into_iter()
    .filter(|snapshot| !before.contains(snapshot.snapshot_id.as_str()))
    .collect::<Vec<_>>();
    reconnect_source(backend, &request.source_provider_ref).await;
    if candidates.is_empty() {
        return Ok(BackendSnapshotRecovery::InProgress);
    }
    if candidates.len() == 1 {
        let candidate = candidates
            .pop()
            .ok_or_else(|| Error::internal_message("snapshot recovery candidate disappeared"))?;
        return Ok(BackendSnapshotRecovery::Recovered(mapping::ready_snapshot(
            candidate.snapshot_id,
        )));
    }
    Ok(BackendSnapshotRecovery::ReconciliationRequired)
}

async fn reconnect_source(backend: &E2bSandboxBackend, source_provider_ref: &ProviderRef) {
    if backend
        .control
        .connect_sandbox(source_provider_ref.as_str())
        .await
        .is_err()
    {
        tracing::warn!(
            backend_id = %backend.config.backend_id(),
            "E2B source reconnect after snapshot did not complete"
        );
    }
}

pub(super) async fn inspect(
    backend: &E2bSandboxBackend,
    request: BackendInspectSnapshotRequest,
) -> Result<BackendSnapshot> {
    let snapshot = control_result(
        backend,
        backend
            .control
            .get_snapshot(
                request.source_provider_ref.as_str(),
                &request.correlation_name,
                request.provider_ref.as_str(),
            )
            .await,
    )?;
    Ok(mapping::ready_snapshot(snapshot.snapshot_id))
}

pub(super) async fn delete(backend: &E2bSandboxBackend, provider_ref: ProviderRef) -> Result<()> {
    control_result(
        backend,
        backend.control.delete_snapshot(provider_ref.as_str()).await,
    )
}

fn map_control(backend: &E2bSandboxBackend, error: AdapterError) -> Error {
    mapping::control(
        error,
        backend.config.backend_id(),
        Some(ResourceKind::Snapshot),
    )
}

fn control_result<T>(backend: &E2bSandboxBackend, result: crate::error::Result<T>) -> Result<T> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => Err(map_control(backend, error)),
    }
}
