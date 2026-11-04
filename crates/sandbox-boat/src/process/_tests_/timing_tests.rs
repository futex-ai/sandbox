//! Virtual-clock idle, absolute-budget, and consumer-drop regressions.

use std::time::Duration;

use futures_util::StreamExt;
use sandbox_interface::{ProcessStreamEvent, ProcessStreamOutcome};
use serde_json::json;

use crate::runtime::configured::BoatRuntime;

use super::timing_support::{Mode, fixture, line, request, start_offset};

#[tokio::test(start_paused = true)]
async fn silence_before_start_expires_idle_and_stops_the_outer_launcher() {
    let fixture = fixture(Mode::Silent);
    let events = fixture
        .runtime
        .stream_process(request(10, 1))
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await;
    assert_eq!(
        events,
        vec![ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::IdleTimeout
        )]
    );
    assert_eq!(
        fixture
            .state
            .lock()
            .unwrap()
            .actions
            .iter()
            .filter(|a| *a == "terminate")
            .count(),
        1
    );
}

#[tokio::test(start_paused = true)]
async fn expiry_after_decoded_child_exit_is_transport_failure_without_a_child_kill() {
    let fixture = fixture(Mode::Ended);
    let events = fixture
        .runtime
        .stream_process(request(10, 1))
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await;
    assert_eq!(
        events,
        vec![
            ProcessStreamEvent::Started { pid: 123 },
            ProcessStreamEvent::Exited {
                exit_code: 0,
                exited: true
            },
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::TransportFailure)
        ]
    );
    assert!(
        !fixture
            .state
            .lock()
            .unwrap()
            .actions
            .contains(&"terminate".to_owned())
    );
}

#[tokio::test(start_paused = true)]
async fn heartbeat_never_resets_idle_and_fresh_output_never_extends_the_absolute_budget() {
    let fixture = fixture(Mode::Channel);
    let mut events = fixture.runtime.stream_process(request(3, 2)).await.unwrap();
    assert_eq!(
        events.next().await,
        Some(ProcessStreamEvent::Started { pid: 123 })
    );
    tokio::time::advance(Duration::from_secs(1)).await;
    fixture
        .input
        .send(Ok(line(json!({"type":"heartbeat"}))))
        .await
        .unwrap();
    let attempt = fixture.state.lock().unwrap().attempt.clone();
    let data = line(json!({"version":1,"attempt":attempt,"kind":"stdout","data":"eA=="}));
    let offset = start_offset(&attempt) + data.len();
    fixture
        .input
        .send(Ok(line(
            json!({"type":"stdout","data":std::str::from_utf8(&data).unwrap(),"nextOffset":offset}),
        )))
        .await
        .unwrap();
    assert_eq!(
        events.next().await,
        Some(ProcessStreamEvent::Stdout(b"x".to_vec()))
    );
    let remaining = events.collect::<Vec<_>>().await;
    assert_eq!(
        remaining,
        vec![ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::DeadlineExpired
        )]
    );
}

#[tokio::test(start_paused = true)]
async fn eof_without_any_success_marker_is_transport_failure() {
    let fixture = fixture(Mode::Channel);
    let mut events = fixture
        .runtime
        .stream_process(request(10, 2))
        .await
        .unwrap();
    assert_eq!(
        events.next().await,
        Some(ProcessStreamEvent::Started { pid: 123 })
    );
    drop(fixture.input);
    assert_eq!(
        events.next().await,
        Some(ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::TransportFailure
        ))
    );
    assert_eq!(events.next().await, None);
}

#[tokio::test(start_paused = true)]
async fn heartbeats_and_slow_consumer_delivery_cannot_postpone_idle_cleanup() {
    let fixture = fixture(Mode::Channel);
    let mut events = fixture
        .runtime
        .stream_process(request(10, 2))
        .await
        .unwrap();
    assert_eq!(
        events.next().await,
        Some(ProcessStreamEvent::Started { pid: 123 })
    );
    let attempt = fixture.state.lock().unwrap().attempt.clone();
    let data = (0..20)
        .flat_map(|_| line(json!({"version":1,"attempt":attempt,"kind":"stdout","data":"eA=="})))
        .collect::<Vec<_>>();
    let offset = start_offset(&attempt) + data.len();
    fixture
        .input
        .send(Ok(line(
            json!({"type":"stdout","data":std::str::from_utf8(&data).unwrap(),"nextOffset":offset}),
        )))
        .await
        .unwrap();
    fixture.consumed.notified().await;
    fixture
        .input
        .send(Ok(line(json!({"type":"heartbeat"}))))
        .await
        .unwrap();
    fixture.consumed.notified().await;
    tokio::time::advance(Duration::from_secs(1)).await;
    fixture
        .input
        .send(Ok(line(json!({"type":"heartbeat"}))))
        .await
        .unwrap();
    fixture.consumed.notified().await;
    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::time::timeout(Duration::from_nanos(1), fixture.terminated.notified())
        .await
        .unwrap();
    let remaining = events.collect::<Vec<_>>().await;
    assert_eq!(remaining.len(), 21);
    assert_eq!(
        remaining.last(),
        Some(&ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::IdleTimeout
        ))
    );
}

#[tokio::test(start_paused = true)]
async fn an_oversized_unterminated_frame_fails_before_provider_eof_or_idle_expiry() {
    let fixture = fixture(Mode::Channel);
    let mut events = fixture
        .runtime
        .stream_process(request(10, 2))
        .await
        .unwrap();
    assert_eq!(
        events.next().await,
        Some(ProcessStreamEvent::Started { pid: 123 })
    );
    fixture
        .input
        .send(Ok(vec![b'x'; 1024 * 1024 + 1]))
        .await
        .unwrap();
    let remaining = events.collect::<Vec<_>>().await;
    assert_eq!(
        remaining,
        vec![ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::TransportFailure
        )]
    );
}
