//! Alternate-backend image-realization failure coverage.

use sandbox_interface::{
    BackendRealizeImageRequest, Error, OperationId, ResourceOwner, SandboxBackend, SandboxId,
    SnapshotId,
};

use super::alternate_backend::AlternateBackend;

#[tokio::test]
async fn alternate_backend_realize_image_failure_may_omit_retained_sandbox() {
    let backend = AlternateBackend::default();
    let error = backend
        .realize_image(BackendRealizeImageRequest {
            sandbox_id: SandboxId::new(),
            snapshot_id: SnapshotId::new(),
            operation_id: OperationId::new(),
            owner: ResourceOwner::platform(uuid::Uuid::now_v7()),
            deployment_id: "backend-conformance".to_owned(),
            profile: "alternate".to_owned(),
            parent_image_provider_ref: None,
            input_files: Vec::new(),
            setup_script: "infra-error".to_owned(),
            verify_commands: vec!["true".to_owned()],
            correlation_name: format!("sandbox-conformance-image-{}", OperationId::new()),
        })
        .await
        .expect_err("infrastructure failure may omit retained sandbox");

    assert!(matches!(error, Error::BackendUnavailable { .. }));
}

#[tokio::test]
async fn alternate_backend_may_omit_command_detail_and_retained_runtime() {
    let backend = AlternateBackend::default();
    let error = backend
        .realize_image(BackendRealizeImageRequest {
            sandbox_id: SandboxId::new(),
            snapshot_id: SnapshotId::new(),
            operation_id: OperationId::new(),
            owner: ResourceOwner::platform(uuid::Uuid::now_v7()),
            deployment_id: "backend-conformance".to_owned(),
            profile: "alternate".to_owned(),
            parent_image_provider_ref: None,
            input_files: Vec::new(),
            setup_script: "typed-without-details".to_owned(),
            verify_commands: vec!["true".to_owned()],
            correlation_name: format!("sandbox-conformance-image-{}", OperationId::new()),
        })
        .await
        .expect_err("typed command failure may omit optional backend detail");

    assert!(matches!(
        error,
        Error::ImageSetupFailed {
            command: None,
            retained_sandbox: None,
        }
    ));
}
