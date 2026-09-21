//! Screen resize acknowledgment and bounded-output coverage.

use std::{sync::Arc, time::Duration};

use sandbox_interface::{
    BackendResizeScreenStackRequest, Error, ProviderRef, SandboxBackend, ScreenViewportSize,
};
use unimock::{MockFn, Unimock, matching};

use crate::backend::configured::E2bSandboxBackend;
use crate::{ProcessSplitOutput, ProcessTransportMock};

use super::support::{config, control, split_success};

#[tokio::test]
async fn resize_forwards_and_requires_the_exact_acknowledgment() {
    let viewport = ScreenViewportSize::new(390, 700).expect("supported viewport");
    let processes = Unimock::new(
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .answers(&|_, _, command| {
                assert_eq!(command.command, "/usr/local/bin/sandbox-screen");
                assert_eq!(command.args, ["resize", "390", "700"]);
                assert_eq!(command.stdout_limit, 4096);
                assert_eq!(command.stderr_limit, 64 * 1024);
                assert_eq!(command.deadline, Duration::from_secs(15));
                Ok(split_success(
                    br#"{"version":1,"width":390,"height":700}"#,
                    b"resize complete\n",
                ))
            }),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control()), Arc::new(processes));

    let applied = backend
        .resize_screen_stack(BackendResizeScreenStackRequest {
            sandbox_provider_ref: ProviderRef::new("provider"),
            viewport,
            deadline_at: None,
        })
        .await
        .expect("exact resize acknowledgment");

    assert_eq!(applied, viewport);
}

#[tokio::test]
async fn mismatched_resize_acknowledgment_is_unavailable() {
    let viewport = ScreenViewportSize::new(1280, 800).expect("supported viewport");
    let processes = Unimock::new(
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .returns(Ok(split_success(
                br#"{"version":1,"width":1279,"height":800}"#,
                b"",
            ))),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control()), Arc::new(processes));

    let error = backend
        .resize_screen_stack(BackendResizeScreenStackRequest {
            sandbox_provider_ref: ProviderRef::new("provider"),
            viewport,
            deadline_at: None,
        })
        .await
        .expect_err("mismatched acknowledgment must fail");

    assert!(matches!(error, Error::BackendUnavailable { .. }));
}

#[tokio::test]
async fn helper_stream_overflow_is_unavailable() {
    for (stdout_overflowed, stderr_overflowed) in [(true, false), (false, true)] {
        let output = ProcessSplitOutput {
            stdout: br#"{"version":1,"width":390,"height":700}"#.to_vec(),
            stderr: b"bounded diagnostic".to_vec(),
            exit_code: Some(0),
            exited: true,
            stdout_overflowed,
            stderr_overflowed,
        };
        let processes = Unimock::new(
            ProcessTransportMock::run_split
                .next_call(matching!(_, _))
                .returns(Ok(output)),
        );
        let backend =
            E2bSandboxBackend::with_transports(config(), Arc::new(control()), Arc::new(processes));
        let viewport = ScreenViewportSize::new(390, 700).expect("supported viewport");

        let error = backend
            .resize_screen_stack(BackendResizeScreenStackRequest {
                sandbox_provider_ref: ProviderRef::new("provider"),
                viewport,
                deadline_at: None,
            })
            .await
            .expect_err("overflowed helper output must fail closed");

        assert!(matches!(error, Error::BackendUnavailable { .. }));
    }
}

#[tokio::test]
async fn resize_rejects_acknowledgment_from_a_signalled_helper() {
    let viewport = ScreenViewportSize::new(390, 700).expect("supported viewport");
    let processes = Unimock::new(
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .returns(Ok(ProcessSplitOutput {
                stdout: br#"{"version":1,"width":390,"height":700}"#.to_vec(),
                exit_code: Some(0),
                exited: false,
                ..ProcessSplitOutput::default()
            })),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control()), Arc::new(processes));

    let result = backend
        .resize_screen_stack(BackendResizeScreenStackRequest {
            sandbox_provider_ref: ProviderRef::new("provider"),
            viewport,
            deadline_at: None,
        })
        .await;

    assert!(matches!(result, Err(Error::BackendUnavailable { .. })));
}
