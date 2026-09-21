//! Image request preflight and size-measurement regressions.

use std::{collections::HashMap, fs, os::unix::fs::PermissionsExt, process::Command, sync::Arc};

use sandbox_interface::{
    BackendPrepareImageRequest, Error, ProviderRef, RealizeImageFileInput, ResourceOwner,
    SandboxBackend, SandboxId,
};
use tempfile::tempdir;
use unimock::Unimock;
use uuid::Uuid;

use crate::{E2bAdapterConfig, E2bProfile, E2bSandboxBackend};

use super::SIZE_COMMAND;

#[tokio::test]
async fn every_image_path_fails_before_provider_access() {
    let backend = E2bSandboxBackend::with_transports(
        config(),
        Arc::new(Unimock::new(())),
        Arc::new(Unimock::new(())),
    );

    let error = backend
        .prepare_image(BackendPrepareImageRequest {
            sandbox_id: SandboxId::new(),
            source_provider_ref: ProviderRef::new("untouched-source"),
            owner: ResourceOwner::platform(Uuid::now_v7()),
            input_files: vec![
                RealizeImageFileInput {
                    root: "/tmp".to_owned(),
                    path: "valid.txt".to_owned(),
                    bytes: b"valid".to_vec(),
                },
                RealizeImageFileInput {
                    root: "/tmp".to_owned(),
                    path: "../outside.txt".to_owned(),
                    bytes: b"invalid".to_vec(),
                },
            ],
            setup_script: "true".to_owned(),
            verify_commands: Vec::new(),
        })
        .await
        .expect_err("invalid later path must fail before connecting");

    assert!(matches!(error, Error::InvalidFilePath { field: "path" }));
}

#[test]
fn size_command_rejects_partial_output_from_failed_du() {
    let tools = tempdir().expect("temporary command directory");
    let du = tools.path().join("du");
    fs::write(&du, "#!/bin/sh\nprintf '4096\\t/\\n'\nexit 1\n").expect("fake du");
    let mut permissions = fs::metadata(&du).expect("fake du metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&du, permissions).expect("executable fake du");

    let output = Command::new("/bin/sh")
        .args(["-c", SIZE_COMMAND])
        .env("PATH", format!("{}:/usr/bin:/bin", tools.path().display()))
        .output()
        .expect("run image size command");

    assert!(
        !output.status.success(),
        "failed du was reported as success"
    );
    assert!(output.stdout.is_empty(), "failed du emitted a size marker");
}

fn config() -> E2bAdapterConfig {
    E2bAdapterConfig::new(
        "configured-e2b",
        "https://api.e2b.app",
        "api-key",
        HashMap::from([(
            "general".to_owned(),
            E2bProfile {
                template: "base".to_owned(),
                allow_public_egress: false,
                denied_destinations: vec!["203.0.113.10/32".to_owned()],
            },
        )]),
        600,
    )
    .expect("valid adapter config")
}
