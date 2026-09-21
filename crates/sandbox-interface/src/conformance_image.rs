//! Caller-driven image phase coverage for the reusable backend harness.

use uuid::Uuid;

use crate::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendPrepareImageRequest,
    BackendSnapshotCreateOutcome, Error, OperationId, RealizeImageFileInput, ResourceOwner, Result,
    SandboxBackend, SandboxId, SandboxNetworkPolicy, SnapshotId,
};

pub(crate) async fn exercise(
    backend: &dyn SandboxBackend,
    profile: &str,
    workspace_id: Uuid,
) -> Result<()> {
    let sandbox_id = SandboxId::new();
    let source = backend
        .create_sandbox(source_request(sandbox_id, profile, workspace_id))
        .await?;
    let prepared = backend
        .prepare_image(BackendPrepareImageRequest {
            sandbox_id,
            source_provider_ref: source.provider_ref.clone(),
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
    if prepared.source_provider_ref != source.provider_ref || prepared.size_bytes == 0 {
        return Err(Error::internal_message(
            "backend image preparation changed its source or returned a zero size",
        ));
    }
    let correlation_name = format!("sandbox-conformance-image-{}", OperationId::new());
    let before = backend
        .snapshot_inventory(source.provider_ref.clone(), correlation_name.clone())
        .await?;
    let snapshot_request = BackendCreateSnapshotRequest {
        snapshot_id: SnapshotId::new(),
        operation_id: OperationId::new(),
        source_provider_ref: source.provider_ref.clone(),
        correlation_name,
        before,
    };
    let BackendSnapshotCreateOutcome::Created(image) =
        backend.create_snapshot(snapshot_request).await?
    else {
        return Err(Error::internal_message(
            "backend conformance image snapshot did not complete",
        ));
    };
    backend.delete_snapshot(image.provider_ref).await?;
    backend.destroy_sandbox(source.provider_ref).await?;
    exercise_failed_preparation(backend, profile, workspace_id).await
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
    let failure = backend
        .prepare_image(BackendPrepareImageRequest {
            sandbox_id,
            source_provider_ref: source.provider_ref.clone(),
            owner: ResourceOwner::platform(workspace_id),
            input_files: Vec::new(),
            setup_script: "exit 7".to_owned(),
            verify_commands: vec!["true".to_owned()],
        })
        .await
        .expect_err("backend setup failure should be typed");
    match failure {
        Error::ImageSetupFailed {
            command: _,
            retained_sandbox: Some(retained_sandbox),
        } if retained_sandbox.sandbox_id == sandbox_id
            && retained_sandbox.provider_ref == source.provider_ref => {}
        _ => {
            return Err(Error::internal_message(
                "backend setup failure did not retain its caller-owned source",
            ));
        }
    }
    backend.destroy_sandbox(source.provider_ref).await
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
