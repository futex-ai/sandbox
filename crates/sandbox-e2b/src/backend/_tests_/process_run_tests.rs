//! Pre-dispatch validation coverage for bounded process execution.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use sandbox_interface::{
    BackendRunProcessRequest, Error, PROCESS_RUN_MAX_ARGV_BYTES, PROCESS_RUN_MAX_DEADLINE,
    PROCESS_RUN_MAX_STREAM_BYTES, ProcessRunContextError, ProviderRef, SandboxBackend,
};
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, ProcessSplitOutput,
    ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn empty_command_is_rejected_before_provider_dispatch() {
    let mut request = valid_request();
    request.command.clear();

    let error = rejected(request).await;

    assert!(matches!(error, Error::EmptyText { field: "command" }));
}

#[tokio::test]
async fn oversized_combined_argv_is_rejected_before_provider_dispatch() {
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
async fn oversized_stream_limits_are_rejected_before_provider_dispatch() {
    let mut stdout_request = valid_request();
    stdout_request.stdout_limit = PROCESS_RUN_MAX_STREAM_BYTES + 1;
    let stdout_error = rejected(stdout_request).await;

    assert!(matches!(
        stdout_error,
        Error::InvalidLength {
            field: "stdout_limit",
            minimum: 0,
            maximum: PROCESS_RUN_MAX_STREAM_BYTES,
        }
    ));

    let mut stderr_request = valid_request();
    stderr_request.stderr_limit = PROCESS_RUN_MAX_STREAM_BYTES + 1;
    let stderr_error = rejected(stderr_request).await;

    assert!(matches!(
        stderr_error,
        Error::InvalidLength {
            field: "stderr_limit",
            minimum: 0,
            maximum: PROCESS_RUN_MAX_STREAM_BYTES,
        }
    ));
}

#[tokio::test]
async fn unbounded_deadline_is_rejected_without_panicking_or_dispatching() {
    let mut request = valid_request();
    request.deadline = Duration::MAX;

    let error = rejected(request).await;

    assert!(matches!(
        error,
        Error::InvalidSeconds {
            field: "deadline",
            minimum: 0,
            maximum: 300,
        }
    ));
}

#[tokio::test]
async fn invalid_execution_context_is_rejected_before_provider_dispatch() {
    let mut cwd_request = valid_request();
    cwd_request.cwd = Some("relative".to_owned());
    let cwd_error = rejected(cwd_request).await;
    assert!(matches!(
        cwd_error,
        Error::InvalidProcessRunContext {
            reason: ProcessRunContextError::InvalidWorkingDirectory
        }
    ));

    let mut env_request = valid_request();
    env_request
        .envs
        .insert("PATH".to_owned(), "secret-path".to_owned());
    let env_error = rejected(env_request).await;
    assert!(matches!(
        env_error,
        Error::InvalidProcessRunContext {
            reason: ProcessRunContextError::TemplateOwnedEnvironmentName { ref name }
        } if name == "PATH"
    ));
    assert!(!env_error.to_string().contains("secret-path"));
}

#[tokio::test]
async fn valid_execution_context_is_forwarded_to_the_process_transport() {
    let control = Arc::new(Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("provider"))
            .returns(Ok(ControlSandboxAccess {
                sandbox_id: "provider".to_owned(),
                domain: "untrusted.example".to_owned(),
                envd_access_token: "access-token".to_owned(),
                traffic_access_token: Some("traffic-token".to_owned()),
            })),
    ));
    let processes = Arc::new(Unimock::new(
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .answers(&|_, connection, command| {
                assert_eq!(connection.sandbox_domain(), "e2b.app");
                assert_eq!(command.cwd.as_deref(), Some("/workspace/repo"));
                assert_eq!(
                    command.envs,
                    BTreeMap::from([("SANDBOX_PROBE".to_owned(), "adapter-value".to_owned())])
                );
                Ok(ProcessSplitOutput {
                    exit_code: Some(0),
                    exited: true,
                    ..ProcessSplitOutput::default()
                })
            }),
    ));
    let backend = E2bSandboxBackend::with_transports(config(), control, processes);
    let mut request = valid_request();
    request.cwd = Some("/workspace/repo".to_owned());
    request.envs = BTreeMap::from([("SANDBOX_PROBE".to_owned(), "adapter-value".to_owned())]);

    let output = backend
        .run_process(request)
        .await
        .expect("valid process context should run");

    assert_eq!(output.exit_code, Some(0));
    assert!(output.exited);
}

async fn rejected(request: BackendRunProcessRequest) -> Error {
    backend()
        .run_process(request)
        .await
        .expect_err("invalid process request should fail")
}

fn valid_request() -> BackendRunProcessRequest {
    BackendRunProcessRequest {
        sandbox_provider_ref: ProviderRef::new("provider"),
        command: "printf".to_owned(),
        args: vec!["ok".to_owned()],
        cwd: None,
        envs: Default::default(),
        stdout_limit: 4096,
        stderr_limit: 4096,
        deadline: PROCESS_RUN_MAX_DEADLINE,
    }
}

fn backend() -> E2bSandboxBackend {
    E2bSandboxBackend::with_transports(
        config(),
        Arc::new(Unimock::new(())),
        Arc::new(Unimock::new(())),
    )
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
