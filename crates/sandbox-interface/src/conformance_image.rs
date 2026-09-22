//! Caller-driven image phase coverage for the reusable backend harness.

use uuid::Uuid;

use crate::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendPrepareImageRequest,
    BackendSnapshot, Error, OperationId, ProviderRef, RealizeImageFileInput, ResourceOwner, Result,
    SandboxBackend, SandboxConsumer, SandboxId, SandboxNetworkPolicy, SnapshotId,
    conformance_resources::{ConformanceResources, TokioRecoverySleeper, finish},
};

pub(crate) async fn exercise(
    backend: &dyn SandboxBackend,
    profile: &str,
    workspace_id: Uuid,
) -> Result<()> {
    let sleeper = TokioRecoverySleeper;
    let mut resources = ConformanceResources::new(backend, &sleeper);
    let outcome = exercise_with_resources(backend, &mut resources, profile, workspace_id).await;
    let cleanup = resources.cleanup().await;
    finish(outcome, cleanup)
}

async fn exercise_with_resources(
    backend: &dyn SandboxBackend,
    resources: &mut ConformanceResources<'_>,
    profile: &str,
    workspace_id: Uuid,
) -> Result<()> {
    let sandbox_id = SandboxId::new();
    let source = resources
        .create_sandbox(source_request(sandbox_id, profile, workspace_id))
        .await?;
    prepare_and_snapshot(
        backend,
        resources,
        sandbox_id,
        &source.provider_ref,
        workspace_id,
    )
    .await?;
    exercise_failed_preparation_with_resources(backend, resources, profile, workspace_id).await
}

async fn prepare_and_snapshot(
    backend: &dyn SandboxBackend,
    resources: &mut ConformanceResources<'_>,
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
    resources
        .create_snapshot(BackendCreateSnapshotRequest {
            snapshot_id: SnapshotId::new(),
            operation_id: OperationId::new(),
            source_provider_ref: source_provider_ref.clone(),
            correlation_name,
            before,
        })
        .await
}

#[cfg(test)]
pub(crate) async fn exercise_failed_preparation(
    backend: &dyn SandboxBackend,
    profile: &str,
    workspace_id: Uuid,
) -> Result<()> {
    let sleeper = TokioRecoverySleeper;
    let mut resources = ConformanceResources::new(backend, &sleeper);
    let outcome =
        exercise_failed_preparation_with_resources(backend, &mut resources, profile, workspace_id)
            .await;
    let cleanup = resources.cleanup().await;
    finish(outcome, cleanup)
}

async fn exercise_failed_preparation_with_resources(
    backend: &dyn SandboxBackend,
    resources: &mut ConformanceResources<'_>,
    profile: &str,
    workspace_id: Uuid,
) -> Result<()> {
    let sandbox_id = SandboxId::new();
    let source = resources
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
        consumer: SandboxConsumer::Runtime,
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
#[path = "_tests_/conformance_snapshot_recovery_tests.rs"]
mod conformance_snapshot_recovery_tests;

#[cfg(test)]
#[path = "_tests_/conformance_sandbox_recovery_tests.rs"]
mod conformance_sandbox_recovery_tests;
