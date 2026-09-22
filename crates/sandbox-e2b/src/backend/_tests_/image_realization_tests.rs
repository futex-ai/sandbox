//! E2B image-preparation behavior tests.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use sandbox_interface::{
    BackendPrepareImageRequest, Error, FILE_TRANSFER_MAX_BYTES, ProviderRef, RealizeImageFileInput,
    ResourceOwner, SandboxBackend, SandboxId,
};
use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, E2bRuntimeConventions,
    ProcessRegularFileWriteRequest, ProcessRunOutput, ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn image_preparation_precedes_separate_source_cleanup() {
    let control = Unimock::new((
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
                assert!(command.args[1].contains("pkill -KILL -f"));
                assert!(command.args[1].contains("pgrep -f"));
                assert!(command.args[1].contains("sandbox_drive_stop_attempt"));
                assert!(command.args[1].contains("rm -rf /tmp/sandbox-drive"));
                Ok(success())
            }),
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(success())),
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

    let prepared = backend
        .prepare_image(BackendPrepareImageRequest {
            sandbox_id: SandboxId::new(),
            source_provider_ref: ProviderRef::new("source"),
            owner: ResourceOwner::platform(Uuid::now_v7()),
            input_files: vec![RealizeImageFileInput {
                root: "/tmp".to_owned(),
                path: "artifact.bin".to_owned(),
                bytes: b"artifact".to_vec(),
            }],
            setup_script: "test -f /tmp/artifact.bin".to_owned(),
            verify_commands: vec!["true".to_owned()],
        })
        .await
        .expect("image source should prepare");

    assert_eq!(prepared.size_bytes, 4096);
    backend
        .destroy_sandbox(prepared.source_provider_ref)
        .await
        .expect("persisted image source should clean up separately");
}

#[tokio::test]
async fn image_scrub_is_safe_for_the_unprivileged_template_user() {
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("scrub-source"))
            .returns(Ok(access("scrub-source"))),
    );
    let processes = Unimock::new((
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(success())),
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .answers(&|_, _, command| {
                let script = command.args.get(1).expect("shell script argument");
                assert!(!script.contains("/root/"));
                assert!(script.contains("tenant-helper"));
                assert!(script.contains("tenant-agent"));
                assert!(script.contains("pkill -TERM -x"));
                assert!(script.contains("pkill -KILL -x"));
                assert!(script.contains("pgrep -x"));
                assert!(script.contains("sandbox_stop_attempt"));
                assert!(script.contains("sandbox_home=\"${HOME:?"));
                assert!(script.contains("-user \"$sandbox_uid\""));
                assert!(script.contains("! -name '.*'"));
                assert!(!script.contains("rm -rf -- \"$sandbox_home/.cache\""));
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
        .prepare_image(BackendPrepareImageRequest {
            sandbox_id: SandboxId::new(),
            source_provider_ref: ProviderRef::new("scrub-source"),
            owner: ResourceOwner::platform(Uuid::now_v7()),
            input_files: Vec::new(),
            setup_script: "true".to_owned(),
            verify_commands: Vec::new(),
        })
        .await
        .expect_err("the test scrub response should fail realization");

    assert!(matches!(error, Error::ImageScrubFailed));
}

#[tokio::test]
async fn oversized_image_input_fails_before_connecting_or_writing_an_earlier_file() {
    let backend = backend(Unimock::new(()), Unimock::new(()));

    let error = backend
        .prepare_image(BackendPrepareImageRequest {
            sandbox_id: SandboxId::new(),
            source_provider_ref: ProviderRef::new("untouched-source"),
            owner: ResourceOwner::platform(Uuid::now_v7()),
            input_files: vec![
                RealizeImageFileInput {
                    root: "/tmp".to_owned(),
                    path: "small.bin".to_owned(),
                    bytes: b"small".to_vec(),
                },
                RealizeImageFileInput {
                    root: "/tmp".to_owned(),
                    path: "oversized.bin".to_owned(),
                    bytes: vec![0; FILE_TRANSFER_MAX_BYTES + 1],
                },
            ],
            setup_script: "true".to_owned(),
            verify_commands: Vec::new(),
        })
        .await
        .expect_err("all inputs should be validated before provider access");

    assert!(matches!(
        error,
        Error::FileTooLarge {
            limit: FILE_TRANSFER_MAX_BYTES
        }
    ));
}

#[tokio::test]
async fn image_commands_never_load_user_login_profiles() {
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("profile-source"))
            .returns(Ok(access("profile-source"))),
    );
    let run_count = Arc::new(AtomicUsize::new(0));
    let processes = Unimock::new(
        ProcessTransportMock::run
            .each_call(matching!(_, _))
            .answers_arc({
                let run_count = run_count.clone();
                Arc::new(move |_, _, command| {
                    if command.command == "/usr/bin/python3" {
                        assert_eq!(command.args.first().map(String::as_str), Some("-I"));
                        assert_eq!(command.args.get(1).map(String::as_str), Some("-S"));
                        run_count.fetch_add(1, Ordering::Relaxed);
                        return Ok(success());
                    }
                    assert_eq!(command.command, "/bin/sh");
                    assert_eq!(command.args.first().map(String::as_str), Some("-c"));
                    assert_eq!(command.args.len(), 2);
                    run_count.fetch_add(1, Ordering::Relaxed);
                    if command.args[1].contains("__SANDBOX_IMAGE_SIZE__=") {
                        return Ok(ProcessRunOutput {
                            bytes: b"__SANDBOX_IMAGE_SIZE__=4096\n".to_vec(),
                            exit_code: Some(0),
                            exited: true,
                            output_truncated: false,
                        });
                    }
                    Ok(success())
                })
            }),
    );

    let prepared = backend(control, processes)
        .prepare_image(BackendPrepareImageRequest {
            sandbox_id: SandboxId::new(),
            source_provider_ref: ProviderRef::new("profile-source"),
            owner: ResourceOwner::platform(Uuid::now_v7()),
            input_files: Vec::new(),
            setup_script: "true".to_owned(),
            verify_commands: vec!["true".to_owned()],
        })
        .await
        .expect("non-login image phases should prepare the source");

    assert_eq!(prepared.size_bytes, 4096);
    assert_eq!(run_count.load(Ordering::Relaxed), 5);
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
