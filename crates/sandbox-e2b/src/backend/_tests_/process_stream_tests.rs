//! Streaming process dispatch and pre-provider validation coverage.

use std::{collections::HashMap, sync::Arc, time::Duration};

use futures_util::{StreamExt, stream};
use sandbox_interface::{
    BackendStreamProcessRequest, Error, PROCESS_RUN_MAX_ARGV_BYTES, PROCESS_RUN_MAX_STREAM_BYTES,
    PROCESS_STREAM_MAX_DEADLINE, ProcessEventStream, ProcessStreamEvent, ProcessStreamOutcome,
    ProviderRef, SandboxBackend,
};
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn stream_process_forwards_validated_request_and_events() {
    let control = Arc::new(Unimock::new(
        E2bControlApiMock::connect_sandbox_with_timeout
            .next_call(matching!("provider", 900))
            .returns(Ok(ControlSandboxAccess {
                sandbox_id: "provider".to_owned(),
                domain: "untrusted.example".to_owned(),
                envd_access_token: "token".to_owned(),
                traffic_access_token: Some("traffic-token".to_owned()),
            })),
    ));
    let processes = Arc::new(Unimock::new(
        ProcessTransportMock::stream_process
            .next_call(matching!(_, _))
            .answers(&|_, connection, command| {
                assert_eq!(connection.sandbox_id(), "provider");
                assert_eq!(connection.sandbox_domain(), "e2b.app");
                assert_eq!(command.command, "/bin/sh");
                assert_eq!(command.args, ["-c", "printf streamed"]);
                assert_eq!(command.stdout_limit, 4096);
                assert_eq!(command.stderr_limit, 1024);
                assert_eq!(command.deadline, Duration::from_secs(900));
                assert_eq!(command.idle_timeout, Duration::from_secs(30));
                Ok(event_stream())
            }),
    ));
    let backend = E2bSandboxBackend::with_transports(config(), control, processes);

    let events = backend
        .stream_process(valid_request())
        .await
        .expect("valid process stream")
        .collect::<Vec<_>>()
        .await;

    assert_eq!(
        events,
        [
            ProcessStreamEvent::Started { pid: 41 },
            ProcessStreamEvent::Stdout(b"streamed".to_vec()),
            ProcessStreamEvent::Exited {
                exit_code: 0,
                exited: true,
            },
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::Completed),
        ]
    );
}

#[tokio::test]
async fn stream_process_rounds_up_deadline_without_shortening_configured_ttl() {
    for (deadline, expected_timeout) in [
        (Duration::from_secs(30), 600),
        (Duration::from_secs(600) + Duration::from_nanos(1), 601),
        (PROCESS_STREAM_MAX_DEADLINE, 3600),
    ] {
        let control = Arc::new(Unimock::new(
            E2bControlApiMock::connect_sandbox_with_timeout
                .next_call(matching!("provider", _))
                .answers_arc(Arc::new(move |_, _, timeout_seconds| {
                    assert_eq!(timeout_seconds, expected_timeout);
                    Ok(ControlSandboxAccess {
                        sandbox_id: "provider".to_owned(),
                        domain: "untrusted.example".to_owned(),
                        envd_access_token: "token".to_owned(),
                        traffic_access_token: Some("traffic-token".to_owned()),
                    })
                })),
        ));
        let processes = Arc::new(Unimock::new(
            ProcessTransportMock::stream_process
                .next_call(matching!(_, _))
                .answers(&|_, _, _| Ok(event_stream())),
        ));
        let backend = E2bSandboxBackend::with_transports(config(), control, processes);
        let mut request = valid_request();
        request.deadline = deadline;
        request.idle_timeout = Duration::from_secs(1);

        backend
            .stream_process(request)
            .await
            .expect("valid process stream")
            .collect::<Vec<_>>()
            .await;
    }
}

#[tokio::test]
async fn stream_process_rejects_argv_and_output_bounds_before_provider_access() {
    let mut empty = valid_request();
    empty.command.clear();
    assert!(matches!(
        rejected(empty).await,
        Error::EmptyText { field: "command" }
    ));

    let mut argv = valid_request();
    argv.args = vec!["a".repeat(PROCESS_RUN_MAX_ARGV_BYTES)];
    assert!(matches!(
        rejected(argv).await,
        Error::CommandTooLarge {
            limit: PROCESS_RUN_MAX_ARGV_BYTES
        }
    ));

    let mut stdout = valid_request();
    stdout.stdout_limit = PROCESS_RUN_MAX_STREAM_BYTES + 1;
    assert!(matches!(
        rejected(stdout).await,
        Error::InvalidLength {
            field: "stdout_limit",
            ..
        }
    ));

    let mut stderr = valid_request();
    stderr.stderr_limit = PROCESS_RUN_MAX_STREAM_BYTES + 1;
    assert!(matches!(
        rejected(stderr).await,
        Error::InvalidLength {
            field: "stderr_limit",
            ..
        }
    ));
}

#[tokio::test]
async fn stream_process_rejects_invalid_timers_before_provider_access() {
    let mut deadline = valid_request();
    deadline.deadline = PROCESS_STREAM_MAX_DEADLINE + Duration::from_nanos(1);
    assert!(matches!(
        rejected(deadline).await,
        Error::InvalidSeconds {
            field: "deadline",
            maximum: 3600,
            ..
        }
    ));

    let mut zero_idle = valid_request();
    zero_idle.idle_timeout = Duration::ZERO;
    assert!(matches!(
        rejected(zero_idle).await,
        Error::InvalidProcessIdleTimeout
    ));

    let mut long_idle = valid_request();
    long_idle.idle_timeout = long_idle.deadline + Duration::from_nanos(1);
    assert!(matches!(
        rejected(long_idle).await,
        Error::InvalidProcessIdleTimeout
    ));
}

async fn rejected(request: BackendStreamProcessRequest) -> Error {
    let result = backend().stream_process(request).await;
    match result {
        Err(error) => error,
        Ok(_) => panic!("invalid process stream request should fail"),
    }
}

fn valid_request() -> BackendStreamProcessRequest {
    BackendStreamProcessRequest {
        sandbox_provider_ref: ProviderRef::new("provider"),
        command: "/bin/sh".to_owned(),
        args: vec!["-c".to_owned(), "printf streamed".to_owned()],
        stdout_limit: 4096,
        stderr_limit: 1024,
        deadline: Duration::from_secs(900),
        idle_timeout: Duration::from_secs(30),
    }
}

fn event_stream() -> ProcessEventStream {
    Box::pin(stream::iter([
        ProcessStreamEvent::Started { pid: 41 },
        ProcessStreamEvent::Stdout(b"streamed".to_vec()),
        ProcessStreamEvent::Exited {
            exit_code: 0,
            exited: true,
        },
        ProcessStreamEvent::Outcome(ProcessStreamOutcome::Completed),
    ]))
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
