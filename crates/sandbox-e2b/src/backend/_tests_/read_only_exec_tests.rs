//! Stateless read-only exec adapter tests.

use std::{collections::HashMap, sync::Arc, time::Duration};

use sandbox_interface::{
    BackendReadOnlyExecRequest, Error, PROCESS_RUN_MAX_ARGV_BYTES, PROCESS_RUN_MAX_STREAM_BYTES,
    ProviderRef, SandboxBackend,
};
use unimock::{MockFn as _, Unimock, matching};

use crate::{
    ControlSandboxReadAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile,
    ProcessOutputCapture, ProcessRunOutput, ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn read_only_exec_forwards_exact_argv_cwd_and_bounds_without_a_pty() {
    let control = Arc::new(Unimock::new(
        E2bControlApiMock::get_sandbox_read_access
            .next_call(matching!("provider"))
            .returns(Ok(ControlSandboxReadAccess {
                sandbox_id: "provider".to_owned(),
                domain: "e2b.app".to_owned(),
                envd_access_token: "token".to_owned(),
            })),
    ));
    let processes = Arc::new(Unimock::new(
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .answers_arc(Arc::new(move |_, connection, command| {
                assert_eq!(connection.sandbox_id(), "provider");
                assert_eq!(command.command, "/usr/bin/git");
                assert_eq!(command.args, ["status", "--short"]);
                assert_eq!(command.cwd.as_deref(), Some("/tmp/sandbox/repo"));
                assert_eq!(
                    command.output_capture,
                    ProcessOutputCapture::HardLimit { max_bytes: 4096 }
                );
                assert_eq!(command.timeout, Duration::from_secs(10));
                assert!(command.read_only);
                Ok(ProcessRunOutput {
                    bytes: b"M src/lib.rs".to_vec(),
                    exit_code: Some(0),
                    exited: true,
                    output_truncated: false,
                })
            })),
    ));
    let backend = E2bSandboxBackend::with_transports(config(), control, processes);

    let output = backend
        .read_only_exec(BackendReadOnlyExecRequest {
            sandbox_provider_ref: ProviderRef::new("provider"),
            cwd: "/tmp/sandbox/repo".to_owned(),
            executable: "/usr/bin/git".to_owned(),
            args: vec!["status".to_owned(), "--short".to_owned()],
            output_limit: 4096,
            timeout: Duration::from_secs(10),
        })
        .await
        .expect("stateless command");

    assert_eq!(output.bytes, b"M src/lib.rs");
    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn read_only_exec_rejects_unexpectedly_truncated_output() {
    let control = Arc::new(Unimock::new(
        E2bControlApiMock::get_sandbox_read_access
            .next_call(matching!("provider"))
            .returns(Ok(ControlSandboxReadAccess {
                sandbox_id: "provider".to_owned(),
                domain: "e2b.app".to_owned(),
                envd_access_token: "token".to_owned(),
            })),
    ));
    let processes = Arc::new(Unimock::new(
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(ProcessRunOutput {
                bytes: b"partial".to_vec(),
                exit_code: Some(0),
                exited: true,
                output_truncated: true,
            })),
    ));
    let backend = E2bSandboxBackend::with_transports(config(), control, processes);

    let error = backend
        .read_only_exec(BackendReadOnlyExecRequest {
            sandbox_provider_ref: ProviderRef::new("provider"),
            cwd: "/tmp/sandbox/repo".to_owned(),
            executable: "/usr/bin/git".to_owned(),
            args: vec!["status".to_owned(), "--short".to_owned()],
            output_limit: 4096,
            timeout: Duration::from_secs(10),
        })
        .await
        .expect_err("truncated read-only output must fail closed");

    assert!(matches!(error, Error::ReadOnlyOutputTooLarge));
}

#[tokio::test]
async fn read_only_exec_rejects_excessive_timeout_before_provider_access() {
    let backend = E2bSandboxBackend::with_transports(
        config(),
        Arc::new(Unimock::new(())),
        Arc::new(Unimock::new(())),
    );

    let error = backend
        .read_only_exec(BackendReadOnlyExecRequest {
            sandbox_provider_ref: ProviderRef::new("provider"),
            cwd: "/tmp/sandbox/repo".to_owned(),
            executable: "/usr/bin/git".to_owned(),
            args: vec!["status".to_owned()],
            output_limit: 4096,
            timeout: Duration::from_secs(301),
        })
        .await
        .expect_err("excessive read-only timeout must be rejected");

    assert!(matches!(
        error,
        Error::InvalidSeconds {
            field: "timeout",
            minimum: 0,
            maximum: 300,
        }
    ));
}

#[tokio::test]
async fn read_only_exec_rejects_an_empty_executable_before_provider_access() {
    let mut request = valid_request();
    request.executable.clear();

    let error = rejected(request).await;

    assert!(matches!(
        error,
        Error::EmptyText {
            field: "executable"
        }
    ));
}

#[tokio::test]
async fn read_only_exec_rejects_oversized_argv_before_provider_access() {
    let mut request = valid_request();
    request.args = vec!["a".repeat(PROCESS_RUN_MAX_ARGV_BYTES)];

    let error = rejected(request).await;

    assert!(matches!(
        error,
        Error::CommandTooLarge {
            limit: PROCESS_RUN_MAX_ARGV_BYTES
        }
    ));
}

#[tokio::test]
async fn read_only_exec_rejects_an_oversized_output_limit_before_provider_access() {
    let mut request = valid_request();
    request.output_limit = PROCESS_RUN_MAX_STREAM_BYTES + 1;

    let error = rejected(request).await;

    assert!(matches!(
        error,
        Error::InvalidLength {
            field: "output_limit",
            minimum: 0,
            maximum: PROCESS_RUN_MAX_STREAM_BYTES,
        }
    ));
}

async fn rejected(request: BackendReadOnlyExecRequest) -> Error {
    E2bSandboxBackend::with_transports(
        config(),
        Arc::new(Unimock::new(())),
        Arc::new(Unimock::new(())),
    )
    .read_only_exec(request)
    .await
    .expect_err("invalid read-only request should fail before provider access")
}

fn valid_request() -> BackendReadOnlyExecRequest {
    BackendReadOnlyExecRequest {
        sandbox_provider_ref: ProviderRef::new("provider"),
        cwd: "/tmp/sandbox/repo".to_owned(),
        executable: "/usr/bin/git".to_owned(),
        args: vec!["status".to_owned()],
        output_limit: 4096,
        timeout: Duration::from_secs(10),
    }
}

fn config() -> E2bAdapterConfig {
    E2bAdapterConfig::new(
        "e2b",
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
    .expect("valid config")
}
