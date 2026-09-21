//! Alternate-backend image-preparation failure coverage.

use sandbox_interface::{
    BackendPrepareImageRequest, Error, ProviderRef, RealizeImageFileInput, ResourceOwner,
    SandboxBackend, SandboxId,
};

use super::alternate_backend::AlternateBackend;

#[tokio::test]
async fn alternate_backend_prepare_image_failure_may_omit_retained_sandbox() {
    let backend = AlternateBackend::default();
    let error = backend
        .prepare_image(BackendPrepareImageRequest {
            sandbox_id: SandboxId::new(),
            source_provider_ref: ProviderRef::new("persisted-source"),
            owner: ResourceOwner::platform(uuid::Uuid::now_v7()),
            input_files: Vec::new(),
            setup_script: "infra-error".to_owned(),
            verify_commands: vec!["true".to_owned()],
        })
        .await
        .expect_err("infrastructure failure may omit retained sandbox");

    assert!(matches!(error, Error::BackendUnavailable { .. }));
}

#[tokio::test]
async fn alternate_backend_may_omit_command_detail_and_retained_runtime() {
    let backend = AlternateBackend::default();
    let error = backend
        .prepare_image(BackendPrepareImageRequest {
            sandbox_id: SandboxId::new(),
            source_provider_ref: ProviderRef::new("persisted-source"),
            owner: ResourceOwner::platform(uuid::Uuid::now_v7()),
            input_files: Vec::new(),
            setup_script: "typed-without-details".to_owned(),
            verify_commands: vec!["true".to_owned()],
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

#[tokio::test]
async fn successful_preparation_returns_the_same_persisted_source() {
    let backend = AlternateBackend::default();
    let prepared = backend
        .prepare_image(BackendPrepareImageRequest {
            sandbox_id: SandboxId::new(),
            source_provider_ref: ProviderRef::new("persisted-source"),
            owner: ResourceOwner::platform(uuid::Uuid::now_v7()),
            input_files: vec![RealizeImageFileInput {
                root: "/tmp".to_owned(),
                path: "sandbox-conformance-input.bin".to_owned(),
                bytes: b"input".to_vec(),
            }],
            setup_script: "test -f /tmp/sandbox-conformance-input.bin".to_owned(),
            verify_commands: vec!["true".to_owned()],
        })
        .await
        .expect("source preparation should succeed");

    assert_eq!(prepared.source_provider_ref.as_str(), "persisted-source");
}
