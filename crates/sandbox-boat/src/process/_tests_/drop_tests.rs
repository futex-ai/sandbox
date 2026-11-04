//! Consumer drop preserves bounded discovery and never kills an unknown PID.

use std::time::Duration;

use futures_util::StreamExt;
use sandbox_interface::ProcessStreamEvent;
use serde_json::json;

use crate::runtime::configured::BoatRuntime;

use super::timing_support::{Mode, fixture, line, request};

#[tokio::test(start_paused = true)]
async fn consumer_drop_after_start_terminates_the_exact_unfinished_attempt() {
    let fixture = fixture(Mode::Running);
    let mut events = fixture
        .runtime
        .stream_process(request(10, 2))
        .await
        .unwrap();
    assert_eq!(
        events.next().await,
        Some(ProcessStreamEvent::Started { pid: 123 })
    );
    drop(events);
    fixture.terminated.notified().await;
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
async fn consumer_drop_preserves_late_pid_grace_past_the_ordinary_idle_deadline() {
    let fixture = fixture(Mode::Late);
    let mut events = fixture
        .runtime
        .stream_process(request(10, 1))
        .await
        .unwrap();
    let pending = tokio::spawn(async move { events.next().await });
    fixture.opened.notified().await;
    fixture.released.notified().await;
    pending.abort();
    let _ = pending.await;
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(2)).await;
    let attempt = fixture.state.lock().unwrap().attempt.clone();
    let hello = line(json!({"version":1,"attempt":attempt,"kind":"hello"}));
    let start = line(json!({"version":1,"attempt":attempt,"kind":"started","pid":123}));
    fixture
        .input
        .send(Ok(line(
            json!({"type":"stdout","data":std::str::from_utf8(&start).unwrap(),
        "nextOffset":hello.len()+start.len()}),
        )))
        .await
        .unwrap();
    fixture.consumed.notified().await;
    tokio::time::timeout(Duration::from_nanos(1), fixture.terminated.notified())
        .await
        .unwrap();
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
async fn drop_without_a_decoded_pid_releases_after_grace_without_any_kill() {
    let fixture = fixture(Mode::UnknownDrop);
    let mut events = fixture
        .runtime
        .stream_process(request(10, 1))
        .await
        .unwrap();
    let pending = tokio::spawn(async move { events.next().await });
    fixture.opened.notified().await;
    fixture.released.notified().await;
    pending.abort();
    let _ = pending.await;
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(3)).await;
    let released = tokio::time::timeout(Duration::from_nanos(1), fixture.input.closed()).await;
    assert!(matches!(released, Ok(())));
    let state = fixture.state.lock().unwrap();
    assert!(!state.actions.contains(&"terminate".to_owned()));
    assert!(!state.actions.contains(&"clean".to_owned()));
}
