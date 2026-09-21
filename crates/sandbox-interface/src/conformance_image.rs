//! Caller-driven image phase coverage for the reusable backend harness.

use std::time::Duration;

use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendPrepareImageRequest,
    BackendSandbox, BackendSnapshot, BackendSnapshotCreateOutcome, BackendSnapshotRecovery, Error,
    OperationId, ProviderRef, RealizeImageFileInput, ResourceOwner, Result, SandboxBackend,
    SandboxId, SandboxNetworkPolicy, SnapshotId,
};

const RECOVERY_INTERVAL: Duration = Duration::from_secs(1);
const RECOVERY_WAITS: usize = 60;

/// Delay boundary used to keep recovery tests deterministic.
#[unimock::unimock(api = RecoverySleeperMock)]
#[async_trait]
trait RecoverySleeper: Send + Sync {
    /// Waits before the next provider recovery poll.
    async fn sleep(&self, duration: Duration);
}

/// Tokio-backed delay used by the public conformance flow.
struct TokioRecoverySleeper;

#[async_trait]
impl RecoverySleeper for TokioRecoverySleeper {
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

pub(crate) async fn exercise(
    backend: &dyn SandboxBackend,
    profile: &str,
    workspace_id: Uuid,
) -> Result<()> {
    let sandbox_id = SandboxId::new();
    let source =
        create_or_recover_sandbox(backend, source_request(sandbox_id, profile, workspace_id))
            .await?;
    let image_result =
        prepare_and_snapshot(backend, sandbox_id, &source.provider_ref, workspace_id).await;
    let image = match image_result {
        Ok(image) => image,
        Err(error) => {
            backend.destroy_sandbox(source.provider_ref).await?;
            return Err(error);
        }
    };
    let delete_result = backend.delete_snapshot(image.provider_ref).await;
    let destroy_result = backend.destroy_sandbox(source.provider_ref).await;
    delete_result?;
    destroy_result?;
    exercise_failed_preparation(backend, profile, workspace_id).await
}

async fn prepare_and_snapshot(
    backend: &dyn SandboxBackend,
    sandbox_id: SandboxId,
    source_provider_ref: &ProviderRef,
    workspace_id: Uuid,
) -> Result<BackendSnapshot> {
    let prepared = backend
        .prepare_image(BackendPrepareImageRequest {
            sandbox_id,
            source_provider_ref: source_provider_ref.clone(),
            owner: ResourceOwner::platform(workspace_id),
            input_files: vec![RealizeImageFileInput {
                root: "/tmp".to_owned(),
                path: "sandbox-conformance-input.bin".to_owned(),
                bytes: b"input".to_vec(),
            }],
            setup_script: "test -f /tmp/sandbox-conformance-input.bin".to_owned(),
            verify_commands: vec!["true".to_owned()],
        })
        .await?;
    if prepared.source_provider_ref != *source_provider_ref || prepared.size_bytes == 0 {
        return Err(Error::internal_message(
            "backend image preparation changed its source or returned a zero size",
        ));
    }
    let correlation_name = format!("sandbox-conformance-image-{}", OperationId::new());
    let before = backend
        .snapshot_inventory(source_provider_ref.clone(), correlation_name.clone())
        .await?;
    let snapshot_request = BackendCreateSnapshotRequest {
        snapshot_id: SnapshotId::new(),
        operation_id: OperationId::new(),
        source_provider_ref: source_provider_ref.clone(),
        correlation_name,
        before,
    };
    create_or_recover_snapshot(backend, snapshot_request).await
}

pub(crate) async fn create_or_recover_snapshot(
    backend: &dyn SandboxBackend,
    request: BackendCreateSnapshotRequest,
) -> Result<BackendSnapshot> {
    create_or_recover_snapshot_with_sleeper(backend, request, &TokioRecoverySleeper).await
}

/// Recovers one dispatched snapshot with bounded, paced polling.
async fn create_or_recover_snapshot_with_sleeper(
    backend: &dyn SandboxBackend,
    request: BackendCreateSnapshotRequest,
    sleeper: &dyn RecoverySleeper,
) -> Result<BackendSnapshot> {
    match backend.create_snapshot(request.clone()).await? {
        BackendSnapshotCreateOutcome::Created(image) => return Ok(image),
        BackendSnapshotCreateOutcome::InProgress
        | BackendSnapshotCreateOutcome::DeliveryAmbiguous => {}
    }
    recover_snapshot_with_sleeper(backend, request, sleeper).await
}

pub(crate) async fn recover_snapshot(
    backend: &dyn SandboxBackend,
    request: BackendCreateSnapshotRequest,
) -> Result<BackendSnapshot> {
    recover_snapshot_with_sleeper(backend, request, &TokioRecoverySleeper).await
}

async fn recover_snapshot_with_sleeper(
    backend: &dyn SandboxBackend,
    request: BackendCreateSnapshotRequest,
    sleeper: &dyn RecoverySleeper,
) -> Result<BackendSnapshot> {
    let mut waits_remaining = RECOVERY_WAITS;
    loop {
        match backend.recover_snapshot_create(request.clone()).await? {
            BackendSnapshotRecovery::Recovered(image) => return Ok(image),
            BackendSnapshotRecovery::InProgress if waits_remaining == 0 => {
                return Err(snapshot_reconciliation_required());
            }
            BackendSnapshotRecovery::InProgress => {
                waits_remaining -= 1;
                sleeper.sleep(RECOVERY_INTERVAL).await;
            }
            BackendSnapshotRecovery::ReconciliationRequired => {
                return Err(snapshot_reconciliation_required());
            }
        }
    }
}

async fn create_or_recover_sandbox(
    backend: &dyn SandboxBackend,
    request: BackendCreateSandboxRequest,
) -> Result<BackendSandbox> {
    create_or_recover_sandbox_with_sleeper(backend, request, &TokioRecoverySleeper).await
}

async fn create_or_recover_sandbox_with_sleeper(
    backend: &dyn SandboxBackend,
    request: BackendCreateSandboxRequest,
    sleeper: &dyn RecoverySleeper,
) -> Result<BackendSandbox> {
    let create_error = match backend.create_sandbox(request.clone()).await {
        Ok(sandbox) => return Ok(sandbox),
        Err(error) => error,
    };
    let mut waits_remaining = RECOVERY_WAITS;
    loop {
        match backend.recover_sandbox_create(request.clone()).await? {
            Some(sandbox) => return Ok(sandbox),
            None if waits_remaining == 0 => return Err(create_error),
            None => {
                waits_remaining -= 1;
                sleeper.sleep(RECOVERY_INTERVAL).await;
            }
        }
    }
}

fn snapshot_reconciliation_required() -> Error {
    Error::SnapshotReconciliationRequired {
        retained_sandbox: None,
    }
}

async fn exercise_failed_preparation(
    backend: &dyn SandboxBackend,
    profile: &str,
    workspace_id: Uuid,
) -> Result<()> {
    let sandbox_id = SandboxId::new();
    let source = backend
        .create_sandbox(source_request(sandbox_id, profile, workspace_id))
        .await?;
    let preparation = backend
        .prepare_image(BackendPrepareImageRequest {
            sandbox_id,
            source_provider_ref: source.provider_ref.clone(),
            owner: ResourceOwner::platform(workspace_id),
            input_files: Vec::new(),
            setup_script: "exit 7".to_owned(),
            verify_commands: vec!["true".to_owned()],
        })
        .await;
    let valid_failure = match preparation {
        Err(Error::ImageSetupFailed {
            command: _,
            retained_sandbox,
        }) => retained_sandbox.is_none_or(|retained_sandbox| {
            retained_sandbox.sandbox_id == sandbox_id
                && retained_sandbox.provider_ref == source.provider_ref
        }),
        _ => false,
    };
    backend.destroy_sandbox(source.provider_ref).await?;
    if !valid_failure {
        return Err(Error::internal_message(
            "backend setup failure returned a mismatched retained source",
        ));
    }
    Ok(())
}

fn source_request(
    sandbox_id: SandboxId,
    profile: &str,
    workspace_id: Uuid,
) -> BackendCreateSandboxRequest {
    BackendCreateSandboxRequest {
        sandbox_id,
        operation_id: OperationId::new(),
        owner: ResourceOwner::platform(workspace_id),
        deployment_id: "backend-conformance".to_owned(),
        profile: profile.to_owned(),
        network: SandboxNetworkPolicy::Open,
        snapshot_provider_ref: None,
    }
}

#[cfg(test)]
#[path = "_tests_/conformance_image_tests.rs"]
mod conformance_image_tests;

#[cfg(test)]
#[path = "_tests_/conformance_sandbox_recovery_tests.rs"]
mod conformance_sandbox_recovery_tests;
