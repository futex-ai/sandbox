//! The streaming budget includes sandbox connection latency at every layer.

use std::{collections::HashMap, sync::Arc, time::Duration};

use futures_util::{StreamExt, stream};
use sandbox_interface::{
    BackendStreamProcessRequest, ProcessStreamEvent, ProcessStreamOutcome, ProviderRef,
    SandboxBackend,
};
use tokio::time::Instant;
use unimock::{MockFn, Unimock, matching};

use crate::process::http::stream_with_timeout as stream_call;
use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, E2bSandboxBackend,
    ProcessTransport,
};

use super::{delayed_control::DelayedControl, support::transport};

#[tokio::test(start_paused = true)]
async fn one_hour_stream_budget_includes_the_control_connection() {
    let processes: Arc<dyn ProcessTransport> = Arc::new(transport(Unimock::new(
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers(&|_, _, _, _, _| Ok(Box::pin(stream::pending()))),
    )));
    let backend = backend(Duration::from_secs(15), 3600, processes);
    let started = Instant::now();

    let events = backend
        .stream_process(request(Duration::from_secs(3600)))
        .await
        .expect("accepted stream")
        .collect::<Vec<_>>()
        .await;

    assert_eq!(Instant::now() - started, Duration::from_secs(3600));
    assert_eq!(events, expired());
}

#[tokio::test(start_paused = true)]
async fn exhausted_connection_budget_returns_deadline_without_starting_a_process() {
    let processes: Arc<dyn ProcessTransport> = Arc::new(transport(Unimock::new(())));
    let backend = backend(Duration::from_secs(20), 600, processes);
    let started = Instant::now();

    let events = backend
        .stream_process(request(Duration::from_secs(10)))
        .await
        .expect("connection expiry is a terminal stream outcome")
        .collect::<Vec<_>>()
        .await;

    assert_eq!(Instant::now() - started, Duration::from_secs(10));
    assert_eq!(events, expired());
}

fn expired() -> Vec<ProcessStreamEvent> {
    vec![ProcessStreamEvent::Outcome(
        ProcessStreamOutcome::DeadlineExpired,
    )]
}

fn request(deadline: Duration) -> BackendStreamProcessRequest {
    BackendStreamProcessRequest {
        sandbox_provider_ref: ProviderRef::new("sandbox"),
        command: "/bin/sh".to_owned(),
        args: vec!["-c".to_owned(), "sleep 3600".to_owned()],
        stdout_limit: 1024,
        stderr_limit: 1024,
        deadline,
        idle_timeout: deadline,
    }
}

fn backend(
    latency: Duration,
    expected_ttl: u32,
    processes: Arc<dyn ProcessTransport>,
) -> E2bSandboxBackend {
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox_with_timeout
            .next_call(matching!("sandbox", _))
            .answers_arc(Arc::new(move |_, _, timeout| {
                assert_eq!(timeout, expected_ttl);
                Ok(ControlSandboxAccess {
                    sandbox_id: "sandbox".to_owned(),
                    domain: "e2b.app".to_owned(),
                    envd_access_token: "token".to_owned(),
                    traffic_access_token: "traffic".to_owned(),
                })
            })),
    );
    let config = E2bAdapterConfig::new(
        "configured-e2b",
        "https://api.e2b.app",
        "test-api-key",
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
    .expect("test configuration");
    E2bSandboxBackend::with_transports(
        config,
        Arc::new(DelayedControl {
            inner: Arc::new(control),
            latency,
        }),
        processes,
    )
}
