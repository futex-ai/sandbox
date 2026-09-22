//! Initialization budgets, remote supervision, and unknown termination coverage.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use sandbox_interface::{
    BackendResizeScreenStackRequest, Error, ProviderRef, SandboxBackend, ScreenViewportSize,
};
use unimock::{MockFn, Unimock, matching};

use crate::backend::configured::E2bSandboxBackend;
use crate::{
    ControlSandboxReadAccess, E2bControlApiMock, ProcessSplitOutput, ProcessTransportMock,
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
    let processes = Unimock::new(
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .answers(&|_, _, command| {
                assert_eq!(command.command, "/usr/bin/timeout");
                assert_eq!(command.args[0], "--signal=KILL");
                let duration: f64 = command.args[1]
                    .trim_end_matches('s')
                    .parse()
                    .expect("timeout seconds");
                assert!(duration > 0.0 && duration <= 15.0);
                assert_eq!(
                    &command.args[2..],
                    ["/usr/local/bin/sandbox-screen", "resize", "704", "768"]
                );
                assert!(
                    command.deadline >= Duration::from_secs_f64(duration) + Duration::from_secs(3)
                );
                Ok(split_success(
                    br#"{"version":1,"width":704,"height":768}"#,
                    b"",
                ))
            }),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control()), Arc::new(processes));
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
async fn a_supervisor_exit_confirms_deadline_cleanup() {
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
    assert!(matches!(
        backend.resize_screen_stack(request()).await,
        Err(Error::BackendUnavailable { .. })
    ));
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
