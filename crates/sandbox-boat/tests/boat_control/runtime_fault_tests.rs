//! Nested protocol failures, reconnect offsets, overflow, and lazy cleanup.

use std::time::Duration;

use futures_util::StreamExt;
use sandbox_interface::{ProcessStreamEvent, ProcessStreamOutcome, SandboxProcesses};

use super::{
    runtime_tests::request,
    support::{fixture::Fixture, runtime_state::Fault},
};

#[tokio::test]
async fn malformed_completion_and_offsets_never_report_completed() {
    let invalid_start = Fixture::start().await;
    invalid_start.state.lock().unwrap().runtime.start_pid = 0;
    let invalid_events = invalid_start
        .backend
        .stream_process(request())
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await;
    assert_eq!(
        invalid_events,
        vec![ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::TransportFailure
        )]
    );
    assert_eq!(
        invalid_start
            .captured()
            .iter()
            .filter(|r| r.path.contains("/stream?"))
            .count(),
        0
    );
    for fault in [
        Fault::MissingSuccess,
        Fault::OffsetGap,
        Fault::WrongAttempt,
        Fault::AfterFinal,
        Fault::SupervisorSignal,
        Fault::MalformedThenEnd,
    ] {
        let fixture = Fixture::start().await;
        fixture.state.lock().unwrap().runtime.fault = fault;
        let events = fixture
            .backend
            .stream_process(request())
            .await
            .unwrap()
            .collect::<Vec<_>>()
            .await;
        assert_eq!(
            events.last(),
            Some(&ProcessStreamEvent::Outcome(
                ProcessStreamOutcome::TransportFailure
            )),
            "{fault:?}"
        );
        let actions = fixture.state.lock().unwrap().runtime.actions.clone();
        if matches!(
            fault,
            Fault::MalformedThenEnd | Fault::OffsetGap | Fault::WrongAttempt
        ) {
            assert!(actions.contains(&"terminate".to_owned()), "{fault:?}");
        } else {
            assert!(!actions.contains(&"terminate".to_owned()), "{fault:?}");
        }
    }
}

#[tokio::test]
async fn reconnect_preserves_partial_inner_frames_and_exact_offsets() {
    let fixture = Fixture::start().await;
    fixture.state.lock().unwrap().runtime.fault = Fault::Reconnect;
    let events = fixture
        .backend
        .stream_process(request())
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await;
    assert_eq!(
        events,
        vec![
            ProcessStreamEvent::Started { pid: 2001 },
            ProcessStreamEvent::Stdout(b"stdout".to_vec()),
            ProcessStreamEvent::Stderr(b"stderr".to_vec()),
            ProcessStreamEvent::Exited {
                exit_code: 0,
                exited: true
            },
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::Completed)
        ]
    );
    assert_eq!(
        fixture
            .captured()
            .iter()
            .filter(|r| r.path.contains("/stream?"))
            .count(),
        2
    );
}

#[tokio::test]
async fn independent_overflow_keeps_only_the_prefix_and_observed_end_prevents_kill() {
    let fixture = Fixture::start().await;
    let mut request = request();
    request.stdout_limit = 3;
    let events = fixture
        .backend
        .stream_process(request)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await;
    assert_eq!(
        events,
        vec![
            ProcessStreamEvent::Started { pid: 2001 },
            ProcessStreamEvent::Stdout(b"std".to_vec()),
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::StdoutOverflow)
        ]
    );
    assert!(
        !fixture
            .state
            .lock()
            .unwrap()
            .runtime
            .actions
            .contains(&"terminate".to_owned())
    );
}

#[tokio::test]
async fn drop_before_lazy_open_dispatches_no_workload_and_removes_transient_state() {
    let fixture = Fixture::start().await;
    let cleaned = fixture.state.lock().unwrap().runtime.cleaned.clone();
    let stream = fixture.backend.stream_process(request()).await.unwrap();
    drop(stream);
    cleaned.notified().await;
    assert_eq!(
        fixture
            .captured()
            .iter()
            .filter(|r| r.body["detached"] == true)
            .count(),
        0
    );
    assert_eq!(fixture.state.lock().unwrap().runtime.staging.len(), 0);
}

#[tokio::test]
async fn consumer_backpressure_terminates_without_waiting_for_delivery_capacity() {
    let fixture = Fixture::start().await;
    fixture.state.lock().unwrap().runtime.fault = Fault::Backpressure;
    let terminated = fixture.state.lock().unwrap().runtime.terminated.clone();
    let mut stream = fixture.backend.stream_process(request()).await.unwrap();
    assert_eq!(
        stream.next().await,
        Some(ProcessStreamEvent::Started { pid: 2001 })
    );
    terminated.notified().await;
    let events = stream.collect::<Vec<_>>().await;
    assert_eq!(
        events.last(),
        Some(&ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::ConsumerBackpressure
        ))
    );
}

#[tokio::test]
async fn process_validation_checks_context_before_argv_and_before_provider_access() {
    let fixture = Fixture::start().await;
    let mut invalid = request();
    invalid.cwd = Some("relative".to_owned());
    invalid.command.clear();
    assert!(matches!(
        fixture.backend.stream_process(invalid).await,
        Err(sandbox_interface::Error::InvalidProcessRunContext { .. })
    ));
    let mut invalid = request();
    invalid.deadline = Duration::from_secs(3601);
    assert!(matches!(
        fixture.backend.stream_process(invalid).await,
        Err(sandbox_interface::Error::InvalidSeconds { maximum: 3600, .. })
    ));
    let mut invalid = request();
    invalid.idle_timeout = Duration::ZERO;
    assert!(matches!(
        fixture.backend.stream_process(invalid).await,
        Err(sandbox_interface::Error::InvalidProcessIdleTimeout)
    ));
    assert_eq!(fixture.captured().len(), 0);
}

#[tokio::test]
async fn drop_of_a_live_https_stream_stops_the_workload_without_command_replay() {
    let fixture = Fixture::start().await;
    fixture.state.lock().unwrap().runtime.fault = Fault::HoldAfterPid;
    let cleaned = fixture.state.lock().unwrap().runtime.cleaned.clone();
    let terminated = fixture.state.lock().unwrap().runtime.terminated.clone();
    let mut stream = fixture.backend.stream_process(request()).await.unwrap();
    assert_eq!(
        stream.next().await,
        Some(ProcessStreamEvent::Started { pid: 2001 })
    );
    drop(stream);
    terminated.notified().await;
    cleaned.notified().await;
    assert_eq!(
        fixture
            .state
            .lock()
            .unwrap()
            .runtime
            .actions
            .iter()
            .filter(|a| *a == "terminate")
            .count(),
        1
    );
    assert_eq!(
        fixture
            .captured()
            .iter()
            .filter(|r| r.body["detached"] == true)
            .count(),
        1
    );
}
