//! Idle and absolute process stream deadline regressions.

use std::{sync::Arc, time::Duration};

use futures_util::{StreamExt, stream};
use sandbox_interface::{ProcessStreamEvent, ProcessStreamOutcome};
use tokio::sync::Notify;
use unimock::{MockFn, Unimock, matching};

use crate::ProcessTransport;
use crate::process::http::{stream_with_timeout as stream_call, unary as unary_call};

use super::super::EVENT_CHANNEL_CAPACITY;
use super::support::{
    byte_stream, command, connection, data_frame, event_frame, start_end_then_pending,
    start_then_periodic, transport,
};

#[tokio::test]
async fn keepalives_do_not_reset_the_idle_timeout() {
    let keepalive = event_frame(r#"{"event":{"keepalive":{}}}"#);
    let transport = transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc(Arc::new(move |_, _, _, _, _| {
                Ok(start_then_periodic(
                    29,
                    keepalive.clone(),
                    Duration::from_millis(5),
                ))
            })),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .returns(Ok(Vec::new())),
    )));

    let stream = transport
        .stream_process(
            connection(),
            command(
                64,
                64,
                Duration::from_millis(200),
                Duration::from_millis(30),
            ),
        )
        .await
        .expect("stream should start");

    assert_eq!(
        stream.collect::<Vec<_>>().await,
        [
            ProcessStreamEvent::Started { pid: 29 },
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::IdleTimeout),
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn buffered_output_does_not_extend_idle_timeout_when_the_consumer_is_slow() {
    let killed = Arc::new(Notify::new());
    let mut frames = vec![event_frame(r#"{"event":{"start":{"pid":43}}}"#)];
    frames.extend((0..18).map(|_| data_frame("stdout", "x")));
    let batch = frames.concat();
    let transport = transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc(Arc::new(move |_, _, _, _, _| {
                Ok(Box::pin(
                    byte_stream(vec![batch.clone()]).chain(stream::pending()),
                ))
            })),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .answers_arc({
                let killed = killed.clone();
                Arc::new(move |_, _, _, _, _| {
                    killed.notify_one();
                    Ok(Vec::new())
                })
            }),
    )));
    let mut events = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_secs(1), Duration::from_millis(20)),
        )
        .await
        .expect("stream should start");

    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(10)).await;
    assert_eq!(
        events.next().await,
        Some(ProcessStreamEvent::Started { pid: 43 })
    );
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(11)).await;
    tokio::time::timeout(Duration::from_millis(1), killed.notified())
        .await
        .expect("old buffered output must not postpone idle cleanup");

    let remaining = events.collect::<Vec<_>>().await;
    assert_eq!(
        remaining.last(),
        Some(&ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::IdleTimeout
        ))
    );
    assert_eq!(remaining.len(), EVENT_CHANNEL_CAPACITY + 1);
    assert!(
        remaining[..EVENT_CHANNEL_CAPACITY]
            .iter()
            .all(|event| matches!(event, ProcessStreamEvent::Stdout(bytes) if bytes == b"x"))
    );
}

#[tokio::test]
async fn stdout_resets_idle_but_not_the_absolute_deadline() {
    let stdout = data_frame("stdout", "x");
    let transport = transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc(Arc::new(move |_, _, _, _, _| {
                Ok(start_then_periodic(
                    31,
                    stdout.clone(),
                    Duration::from_millis(5),
                ))
            })),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .returns(Ok(Vec::new())),
    )));

    let stream = transport
        .stream_process(
            connection(),
            command(
                1024,
                64,
                Duration::from_millis(40),
                Duration::from_millis(20),
            ),
        )
        .await
        .expect("stream should start");
    let events = stream.collect::<Vec<_>>().await;

    assert_eq!(
        events.first(),
        Some(&ProcessStreamEvent::Started { pid: 31 })
    );
    assert!(
        events[1..events.len() - 1]
            .iter()
            .all(|event| matches!(event, ProcessStreamEvent::Stdout(bytes) if bytes == b"x"))
    );
    assert_eq!(
        events.last(),
        Some(&ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::DeadlineExpired
        ))
    );
}

#[tokio::test]
async fn deadline_after_exit_without_a_trailer_is_a_transport_failure() {
    let transport = transport(Unimock::new(
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc(Arc::new(move |_, _, _, _, _| {
                Ok(start_end_then_pending(37))
            })),
    ));

    let stream = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_millis(30), Duration::from_millis(30)),
        )
        .await
        .expect("stream should start");

    assert_eq!(
        stream.collect::<Vec<_>>().await,
        [
            ProcessStreamEvent::Started { pid: 37 },
            ProcessStreamEvent::Exited {
                exit_code: 0,
                exited: true,
            },
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::TransportFailure),
        ]
    );
}

#[tokio::test]
async fn idle_expiry_while_delivering_exit_is_a_transport_failure() {
    let mut events = vec![event_frame(r#"{"event":{"start":{"pid":41}}}"#)];
    events.extend((0..EVENT_CHANNEL_CAPACITY - 1).map(|_| data_frame("stdout", "x")));
    events.push(event_frame(
        r#"{"event":{"end":{"exitCode":0,"exited":true}}}"#,
    ));
    let transport = transport(Unimock::new(
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc(Arc::new(move |_, _, _, _, _| {
                Ok(Box::pin(
                    byte_stream(events.clone()).chain(stream::pending()),
                ))
            })),
    ));
    let stream = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_secs(2), Duration::from_millis(30)),
        )
        .await
        .expect("stream should start");

    tokio::time::sleep(Duration::from_millis(75)).await;
    let events = stream.collect::<Vec<_>>().await;

    assert_eq!(
        events.first(),
        Some(&ProcessStreamEvent::Started { pid: 41 })
    );
    assert!(
        events[1..events.len() - 1]
            .iter()
            .all(|event| matches!(event, ProcessStreamEvent::Stdout(bytes) if bytes == b"x"))
    );
    assert_eq!(events.len(), EVENT_CHANNEL_CAPACITY + 1);
    assert_eq!(
        events.last(),
        Some(&ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::TransportFailure
        ))
    );
}
