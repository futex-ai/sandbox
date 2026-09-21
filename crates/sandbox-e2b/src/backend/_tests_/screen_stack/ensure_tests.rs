//! Screen ensure and capability probe coverage.

use std::{sync::Arc, time::Duration};

use sandbox_interface::{Error, SandboxBackend, ScreenStackCapabilities, ScreenStackOutcome};
use unimock::{MockFn, Unimock, matching};

use crate::backend::configured::E2bSandboxBackend;
use crate::{E2bRuntimeConventions, ProcessTransportMock};

use super::support::{
    CAPABILITIES, backend, backend_with_capabilities, config, control, request, split_failure,
    split_success,
};

#[tokio::test]
async fn already_running_stack_is_ready_without_a_restart_contract() {
    let backend = backend(split_success(b"already-running\n", b""));

    let outcome = backend
        .ensure_screen_stack(request())
        .await
        .expect("already-running stack should be ready");

    assert_eq!(
        outcome,
        ScreenStackOutcome::Ready {
            capabilities: ScreenStackCapabilities::default(),
        }
    );
}

#[tokio::test]
async fn cold_stack_is_started_through_the_exact_helper_command() {
    let processes = Unimock::new((
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .answers(&|_, connection, command| {
                assert_eq!(connection.sandbox_id(), "provider");
                assert_eq!(connection.access_token(), "envd-token");
                assert_eq!(command.command, "/opt/tenant/screen-helper");
                assert_eq!(command.args, ["ensure"]);
                assert_eq!(command.stdout_limit, 4096);
                assert_eq!(command.stderr_limit, 64 * 1024);
                assert_eq!(command.deadline, Duration::from_secs(300));
                Ok(split_success(b"started\n", b""))
            }),
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .answers(&|_, _, command| {
                assert_eq!(command.command, "/opt/tenant/screen-helper");
                assert_eq!(command.args, ["capabilities"]);
                assert_eq!(command.deadline, Duration::from_secs(15));
                Ok(split_success(CAPABILITIES, b""))
            }),
    ));
    let config = config().with_runtime_conventions(
        E2bRuntimeConventions::new("tenant", "tenant-terminal-", "/opt/tenant/screen-helper")
            .unwrap(),
    );
    let backend =
        E2bSandboxBackend::with_transports(config, Arc::new(control()), Arc::new(processes));

    let outcome = backend
        .ensure_screen_stack(request())
        .await
        .expect("cold stack should start");

    assert_eq!(
        outcome,
        ScreenStackOutcome::Ready {
            capabilities: ScreenStackCapabilities {
                dynamic_resize: true,
            },
        }
    );
}

#[tokio::test]
async fn helper_machine_output_is_split_from_bounded_diagnostics() {
    let processes = Unimock::new((
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .answers(&|_, connection, command| {
                assert_eq!(connection.sandbox_id(), "provider");
                assert_eq!(command.command, "/usr/local/bin/sandbox-screen");
                assert_eq!(command.args, ["ensure"]);
                assert_eq!(command.stdout_limit, 4096);
                assert_eq!(command.stderr_limit, 64 * 1024);
                Ok(split_success(b"ready\n", b"screen stack already running\n"))
            }),
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .answers(&|_, _, command| {
                assert_eq!(command.args, ["capabilities"]);
                assert_eq!(command.stdout_limit, 4096);
                assert_eq!(command.stderr_limit, 64 * 1024);
                Ok(split_success(CAPABILITIES, b"capability probe complete\n"))
            }),
    ));
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control()), Arc::new(processes));

    let outcome = backend
        .ensure_screen_stack(request())
        .await
        .expect("stderr diagnostics must not corrupt helper JSON");

    assert_eq!(
        outcome,
        ScreenStackOutcome::Ready {
            capabilities: ScreenStackCapabilities {
                dynamic_resize: true,
            },
        }
    );
}

#[tokio::test]
async fn malformed_or_unknown_capabilities_fall_back_to_static_screen() {
    for output in [b"not-json".as_slice(), br#"{"version":2}"#, br#"{"version":1,"features":{"dynamic_resize":true},"viewport":{"min_width":321,"max_width":3840,"min_height":240,"max_height":2160,"max_pixels":8294400}}"#] {
        let backend = backend_with_capabilities(split_success(output, b""));
        let outcome = backend
            .ensure_screen_stack(request())
            .await
            .expect("legacy screen remains usable");
        assert_eq!(
            outcome,
            ScreenStackOutcome::Ready {
                capabilities: ScreenStackCapabilities::default(),
            }
        );
    }
}

#[tokio::test]
async fn unsuccessful_helper_maps_to_backend_unavailable() {
    let processes = Unimock::new(
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .returns(Ok(split_failure(b"watch bridge failed\n"))),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control()), Arc::new(processes));

    let error = backend
        .ensure_screen_stack(request())
        .await
        .expect_err("unsuccessful ensure should fail");

    assert!(matches!(
        error,
        Error::BackendUnavailable { backend_id } if backend_id == "configured-e2b"
    ));
}

#[tokio::test]
async fn envd_transport_failure_remains_typed() {
    let processes = Unimock::new(
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .returns(Err(Error::BackendUnavailable {
                backend_id: "configured-e2b".to_owned(),
            })),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control()), Arc::new(processes));

    let error = backend
        .ensure_screen_stack(request())
        .await
        .expect_err("transport failure should fail");

    assert!(matches!(
        error,
        Error::BackendUnavailable { backend_id } if backend_id == "configured-e2b"
    ));
}
