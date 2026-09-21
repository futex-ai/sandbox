//! E2B image-realization failure behavior tests.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{
    BackendRealizeImageRequest, Error, OperationId, RealizeImageFileInput, ResourceOwner,
    SandboxBackend, SandboxId, SnapshotId,
};
use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use crate::{
    ControlSandboxAccess, ControlSnapshot, E2bAdapterConfig, E2bControlApiMock, E2bProfile,
    E2bRuntimeConventions, ProcessRegularFileWriteRequest, ProcessRunOutput, ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn image_realization_uploads_input_files_before_setup() {
    let control = Unimock::new((
        E2bControlApiMock::list_sandboxes
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
        E2bControlApiMock::create_sandbox
            .next_call(matching!(_))
            .returns(Ok(access("source"))),
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("source"))
            .returns(Ok(access("source"))),
        E2bControlApiMock::list_snapshots
            .next_call(matching!("source", "sandbox-input-image"))
            .returns(Ok(Vec::new())),
        E2bControlApiMock::create_snapshot
            .next_call(matching!("source", "sandbox-input-image"))
            .returns(Ok(ControlSnapshot {
                snapshot_id: "snapshot".to_owned(),
            })),
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("source"))
            .returns(Ok(access("source"))),
        E2bControlApiMock::kill_sandbox
            .next_call(matching!("source"))
            .returns(Ok(())),
    ));
    let processes = Unimock::new((
        ProcessTransportMock::write_regular_file
            .next_call(matching!(_, _))
            .answers(&|_, _, request: ProcessRegularFileWriteRequest| {
                assert_eq!(request.root, "/tmp");
                assert_eq!(request.path, "artifact.bin");
                assert_eq!(request.bytes, b"artifact");
                Ok(())
            }),
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(success())),
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(success())),
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .answers(&|_, _, command| {
                assert!(command.args[1].contains("fusermount3 -u /drives/me"));
                assert!(command.args[1].contains("[s]andbox-drive-"));
                assert!(command.args[1].contains("rm -rf /tmp/sandbox-drive"));
                Ok(success())
            }),
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .answers(&|_, _, command| {
                assert!(
                    command
                        .args
                        .last()
                        .expect("size shell argument")
                        .contains("__SANDBOX_IMAGE_SIZE__=")
                );
                Ok(ProcessRunOutput {
                    bytes: b"shell warning\n__SANDBOX_IMAGE_SIZE__=4096\n".to_vec(),
                    exit_code: Some(0),
                    exited: true,
                    output_truncated: false,
                })
            }),
    ));
    let backend = backend(control, processes);

    let image = backend
        .realize_image(BackendRealizeImageRequest {
            sandbox_id: SandboxId::new(),
            snapshot_id: SnapshotId::new(),
            operation_id: OperationId::new(),
            owner: ResourceOwner::platform(Uuid::now_v7()),
            deployment_id: "deployment".to_owned(),
            profile: "general".to_owned(),
            parent_image_provider_ref: None,
            input_files: vec![RealizeImageFileInput {
                root: "/tmp".to_owned(),
                path: "artifact.bin".to_owned(),
                bytes: b"artifact".to_vec(),
            }],
            setup_script: "test -f /tmp/artifact.bin".to_owned(),
            verify_commands: vec!["true".to_owned()],
            correlation_name: "sandbox-input-image".to_owned(),
        })
        .await
        .expect("image should realize");

    assert_eq!(image.size_bytes, 4096);
}

#[tokio::test]
async fn image_scrub_is_safe_for_the_unprivileged_template_user() {
    let control = Unimock::new((
        E2bControlApiMock::list_sandboxes
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
        E2bControlApiMock::create_sandbox
            .next_call(matching!(_))
            .returns(Ok(access("scrub-source"))),
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("scrub-source"))
            .returns(Ok(access("scrub-source"))),
        E2bControlApiMock::kill_sandbox
            .next_call(matching!("scrub-source"))
            .returns(Ok(())),
    ));
    let processes = Unimock::new((
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(success())),
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .answers(&|_, _, command| {
                let script = command.args.last().expect("shell script argument");
                assert!(!script.contains("/root/"));
                assert!(script.contains("tenant-helper"));
                assert!(script.contains("tenant-agent"));
                assert!(script.contains("pkill -TERM -x"));
                assert!(script.contains("sandbox_home=\"${HOME:?"));
                assert!(script.contains("-user \"$sandbox_uid\""));
                assert!(script.contains("! -name '.*'"));
                Ok(ProcessRunOutput {
                    bytes: Vec::new(),
                    exit_code: Some(1),
                    exited: true,
                    output_truncated: false,
                })
            }),
    ));
    let conventions = E2bRuntimeConventions::default()
        .with_image_process_names("tenant-helper", "tenant-agent")
        .expect("safe image process names");
    let backend = backend_with_config(
        control,
        processes,
        config().with_runtime_conventions(conventions),
    );

    let error = backend
        .realize_image(BackendRealizeImageRequest {
            sandbox_id: SandboxId::new(),
            snapshot_id: SnapshotId::new(),
            operation_id: OperationId::new(),
            owner: ResourceOwner::platform(Uuid::now_v7()),
            deployment_id: "deployment".to_owned(),
            profile: "general".to_owned(),
            parent_image_provider_ref: None,
            input_files: Vec::new(),
            setup_script: "true".to_owned(),
            verify_commands: Vec::new(),
            correlation_name: "sandbox-scrub-check".to_owned(),
        })
        .await
        .expect_err("the test scrub response should fail realization");

    assert!(matches!(error, Error::ImageScrubFailed));
}

fn backend(control: Unimock, processes: Unimock) -> E2bSandboxBackend {
    backend_with_config(control, processes, config())
}

fn backend_with_config(
    control: Unimock,
    processes: Unimock,
    config: E2bAdapterConfig,
) -> E2bSandboxBackend {
    E2bSandboxBackend::with_transports(config, Arc::new(control), Arc::new(processes))
}

fn success() -> ProcessRunOutput {
    ProcessRunOutput {
        bytes: Vec::new(),
        exit_code: Some(0),
        exited: true,
        output_truncated: false,
    }
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

fn access(sandbox_id: &str) -> ControlSandboxAccess {
    ControlSandboxAccess {
        sandbox_id: sandbox_id.to_owned(),
        domain: "e2b.app".to_owned(),
        envd_access_token: "call-local-token".to_owned(),
        traffic_access_token: "traffic-token".to_owned(),
    }
}
