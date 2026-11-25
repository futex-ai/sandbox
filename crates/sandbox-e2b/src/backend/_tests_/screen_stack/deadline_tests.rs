//! Initialization budgets, remote supervision, and unknown termination coverage.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use sandbox_interface::{
    BackendResizeScreenStackRequest, Error, ProviderRef, SandboxScreens, ScreenViewportSize,
};
use unimock::{MockFn, Unimock, matching};

use crate::backend::configured::E2bSandboxBackend;
use crate::{
    ControlSandboxReadAccess, E2bControlApiMock, E2bRuntimeConventions, ProcessSplitOutput,
    ProcessTransportMock,
};

use super::support::{config, split_success};

#[tokio::test]
async fn expired_initialization_never_connects_or_starts_a_helper() {
    let backend = E2bSandboxBackend::with_transports(
        config(),
        Arc::new(Unimock::new(())),
        Arc::new(Unimock::new(())),
    );
    let mut request = request();
    request.deadline_at = Some((SystemTime::now() - Duration::from_secs(1)).into());
    assert!(matches!(
        backend.resize_screen_stack(request).await,
        Err(Error::BackendUnavailable { .. })
    ));
}

#[tokio::test]
async fn initialization_uses_read_only_credentials_and_reserves_termination_time() {
    let config = config().with_runtime_conventions(
        E2bRuntimeConventions::new("tenant", "tenant-terminal-", "/opt/tenant/screen-helper")
            .expect("valid runtime conventions"),
    );
    let processes = Unimock::new(
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .answers(&|_, _, command| {
                assert_eq!(command.command, "/usr/bin/python3");
                assert_eq!(&command.args[..3], ["-I", "-S", "-c"]);
                let budget_ms: u64 = command.args[4].parse().expect("timeout milliseconds");
                assert!((1..=15_000).contains(&budget_ms));
                assert_eq!(
                    &command.args[5..],
                    ["/opt/tenant/screen-helper", "resize", "704", "768"]
                );
                assert!(
                    command.deadline >= Duration::from_millis(budget_ms) + Duration::from_secs(3)
                );
                Ok(split_success(
                    br#"{"version":1,"width":704,"height":768}"#,
                    b"",
                ))
            }),
    );
    let backend =
        E2bSandboxBackend::with_transports(config, Arc::new(control()), Arc::new(processes));
    assert_eq!(
        backend
            .resize_screen_stack(request())
            .await
            .expect("initialized"),
        request().viewport
    );
}

#[tokio::test]
async fn a_timeout_without_observed_exit_is_not_reported_as_a_safe_failure() {
    for transport_error in [false, true] {
        let processes = Unimock::new(
            ProcessTransportMock::run_split
                .next_call(matching!(_, _))
                .answers_arc(Arc::new(move |_, _, _| {
                    if transport_error {
                        Err(Error::BackendUnavailable {
                            backend_id: "test".to_owned(),
                        })
                    } else {
                        Ok(ProcessSplitOutput::default())
                    }
                })),
        );
        let backend =
            E2bSandboxBackend::with_transports(config(), Arc::new(control()), Arc::new(processes));
        assert!(matches!(
            backend.resize_screen_stack(request()).await,
            Err(Error::ScreenViewportResizeUnconfirmed)
        ));
    }
}

#[tokio::test]
async fn a_signalled_supervisor_leaves_deadline_cleanup_unconfirmed() {
    let processes = Unimock::new(
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .returns(Ok(ProcessSplitOutput {
                exited: false,
                exit_code: Some(-1),
                ..ProcessSplitOutput::default()
            })),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control()), Arc::new(processes));
    let result = backend.resize_screen_stack(request()).await;
    assert!(
        matches!(result, Err(Error::ScreenViewportResizeUnconfirmed)),
        "expected ScreenViewportResizeUnconfirmed after a signal, got {result:?}"
    );
}

#[tokio::test]
async fn known_supervisor_failure_codes_report_backend_unavailable() {
    for (case, exit_code) in [("timeout cleanup", 124), ("helper failure", 3)] {
        let processes = Unimock::new(
            ProcessTransportMock::run_split
                .some_call(matching!(_, _))
                .returns(Ok(ProcessSplitOutput {
                    exited: true,
                    exit_code: Some(exit_code),
                    ..ProcessSplitOutput::default()
                })),
        );
        let backend =
            E2bSandboxBackend::with_transports(config(), Arc::new(control()), Arc::new(processes));
        let result = backend.resize_screen_stack(request()).await;
        assert!(
            matches!(result, Err(Error::BackendUnavailable { .. })),
            "expected BackendUnavailable after {case} with exit code {exit_code}, got {result:?}"
        );
    }
}

#[tokio::test]
async fn unexpected_supervisor_exits_leave_deadline_cleanup_unconfirmed() {
    let empty_stdout = b"".as_slice();
    for (exited, exit_code, stdout) in [
        (true, 1, empty_stdout),
        (true, 2, empty_stdout),
        (true, 120, empty_stdout),
        (true, 137, empty_stdout),
        (true, 255, empty_stdout),
        (false, 124, empty_stdout),
        (false, 0, br#"{"version":1,"width":704,"height":768}"#),
    ] {
        let processes = Unimock::new(
            ProcessTransportMock::run_split
                .some_call(matching!(_, _))
                .returns(Ok(ProcessSplitOutput {
                    exited,
                    exit_code: Some(exit_code),
                    stdout: stdout.to_vec(),
                    ..ProcessSplitOutput::default()
                })),
        );
        let backend =
            E2bSandboxBackend::with_transports(config(), Arc::new(control()), Arc::new(processes));
        let result = backend.resize_screen_stack(request()).await;
        assert!(
            matches!(result, Err(Error::ScreenViewportResizeUnconfirmed)),
            "expected ScreenViewportResizeUnconfirmed for exited={exited} and exit code {exit_code}, got {result:?}"
        );
    }
}

fn request() -> BackendResizeScreenStackRequest {
    BackendResizeScreenStackRequest {
        sandbox_provider_ref: ProviderRef::new("provider"),
        viewport: ScreenViewportSize::new(704, 768).expect("viewport"),
        deadline_at: Some((SystemTime::now() + Duration::from_secs(18)).into()),
    }
}

fn control() -> Unimock {
    Unimock::new(
        E2bControlApiMock::get_sandbox_read_access
            .next_call(matching!(_))
            .returns(Ok(ControlSandboxReadAccess {
                sandbox_id: "provider".to_owned(),
                domain: "e2b.app".to_owned(),
                envd_access_token: "envd".to_owned(),
            })),
    )
}
