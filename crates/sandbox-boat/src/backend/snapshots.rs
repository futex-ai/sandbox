//! Caller-owned capture sources and exact-name recover-only saved snapshots.

use sandbox_interface::{
    BackendCreateSnapshotRequest, BackendInspectSnapshotRequest, BackendSnapshot,
    BackendSnapshotCreateOutcome, BackendSnapshotInventory, BackendSnapshotRecovery, Error,
    ProviderRef, ResourceKind, Result, SandboxConsumer,
};

use crate::{
    control::types::{BoatSaveSnapshot, BoatSavedSnapshot, BoatSavedSnapshotStatus},
    error::Error as AdapterError,
    transport::request::{valid_sandbox_id, valid_snapshot_name},
};

use super::{configured::BoatSandboxBackend, identity, mapping};

pub(super) async fn inventory(
    backend: &BoatSandboxBackend,
    source: ProviderRef,
    name: String,
) -> Result<BackendSnapshotInventory> {
    validate(&source, &name)?;
    let Some(snapshot) = read(backend, &name).await? else {
        return Ok(BackendSnapshotInventory::default());
    };
    ensure_source(&snapshot, &source)?;
    Ok(BackendSnapshotInventory {
        snapshots: vec![ProviderRef::new(snapshot.name)],
    })
}

pub(super) async fn create(
    backend: &BoatSandboxBackend,
    request: BackendCreateSnapshotRequest,
) -> Result<BackendSnapshotCreateOutcome> {
    validate(&request.source_provider_ref, &request.correlation_name)?;
    if !request.before.snapshots.is_empty()
        || read(backend, &request.correlation_name).await?.is_some()
    {
        return Err(reconciliation());
    }
    let source = mapping::control_result(
        backend
            .control
            .get_sandbox(request.source_provider_ref.as_str())
            .await,
        backend.config.backend_id(),
        Some(ResourceKind::Sandbox),
    )?;
    if source
        .labels
        .get(identity::CONSUMER)
        .and_then(|value| SandboxConsumer::from_metadata(value))
        == Some(SandboxConsumer::Browser)
    {
        return Err(Error::RuntimeConsumerRequired {
            resource: ResourceKind::Snapshot,
            consumer: SandboxConsumer::Browser,
        });
    }
    if source.snapshots != Some(true) {
        return mapping::control_result(
            Err(AdapterError::SnapshotCaptureDisabled),
            backend.config.backend_id(),
            Some(ResourceKind::Snapshot),
        );
    }
    let result = backend
        .control
        .save_snapshot(BoatSaveSnapshot {
            sandbox_id: request.source_provider_ref.as_str().to_owned(),
            name: request.correlation_name,
        })
        .await;
    match result {
        Ok(snapshot) => {
            ensure_source(&snapshot, &request.source_provider_ref)?;
            match snapshot.status {
                BoatSavedSnapshotStatus::Saving => Ok(BackendSnapshotCreateOutcome::InProgress),
                BoatSavedSnapshotStatus::Ready | BoatSavedSnapshotStatus::Failed => Ok(
                    BackendSnapshotCreateOutcome::Created(mapping::snapshot(snapshot)),
                ),
            }
        }
        Err(AdapterError::DeliveryAmbiguous) => Ok(BackendSnapshotCreateOutcome::DeliveryAmbiguous),
        Err(error) => mapping::control_result(
            Err(error),
            backend.config.backend_id(),
            Some(ResourceKind::Snapshot),
        ),
    }
}

pub(super) async fn recover(
    backend: &BoatSandboxBackend,
    request: BackendCreateSnapshotRequest,
) -> Result<BackendSnapshotRecovery> {
    validate(&request.source_provider_ref, &request.correlation_name)?;
    if !request.before.snapshots.is_empty() {
        return Ok(BackendSnapshotRecovery::ReconciliationRequired);
    }
    let Some(snapshot) = read(backend, &request.correlation_name).await? else {
        return Ok(BackendSnapshotRecovery::InProgress);
    };
    if snapshot.source_sandbox_id != request.source_provider_ref.as_str() {
        return Ok(BackendSnapshotRecovery::ReconciliationRequired);
    }
    Ok(match snapshot.status {
        BoatSavedSnapshotStatus::Saving => BackendSnapshotRecovery::InProgress,
        BoatSavedSnapshotStatus::Ready | BoatSavedSnapshotStatus::Failed => {
            BackendSnapshotRecovery::Recovered(mapping::snapshot(snapshot))
        }
    })
}

pub(super) async fn inspect(
    backend: &BoatSandboxBackend,
    request: BackendInspectSnapshotRequest,
) -> Result<BackendSnapshot> {
    validate(&request.source_provider_ref, &request.correlation_name)?;
    if request.provider_ref.as_str() != request.correlation_name {
        return Err(reconciliation());
    }
    let snapshot = read(backend, request.provider_ref.as_str())
        .await?
        .ok_or(Error::NotFound {
            resource: ResourceKind::Snapshot,
        })?;
    ensure_source(&snapshot, &request.source_provider_ref)?;
    Ok(mapping::snapshot(snapshot))
}

fn validate(source: &ProviderRef, name: &str) -> Result<()> {
    if !valid_sandbox_id(source.as_str()) || !valid_snapshot_name(name) {
        return Err(Error::internal_message(
            "Boat snapshot source or correlation is invalid",
        ));
    }
    Ok(())
}

async fn read(backend: &BoatSandboxBackend, name: &str) -> Result<Option<BoatSavedSnapshot>> {
    match backend.control.get_snapshot(name).await {
        Ok(snapshot) => Ok(Some(snapshot)),
        Err(AdapterError::NotFound) => Ok(None),
        Err(error) => mapping::control_result(
            Err(error),
            backend.config.backend_id(),
            Some(ResourceKind::Snapshot),
        ),
    }
}

fn ensure_source(snapshot: &BoatSavedSnapshot, source: &ProviderRef) -> Result<()> {
    if snapshot.source_sandbox_id != source.as_str() {
        return Err(reconciliation());
    }
    Ok(())
}

fn reconciliation() -> Error {
    Error::SnapshotReconciliationRequired {
        retained_sandbox: None,
    }
}
