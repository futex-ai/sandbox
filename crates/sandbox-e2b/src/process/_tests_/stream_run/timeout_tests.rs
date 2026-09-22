//! Idle and absolute process stream deadline regressions.

use std::{sync::Arc, time::Duration};

use futures_util::StreamExt;
use sandbox_interface::{ProcessStreamEvent, ProcessStreamOutcome};
use unimock::{MockFn, Unimock, matching};

use crate::ProcessTransport;
use crate::process::http::{stream_with_timeout as stream_call, unary as unary_call};

use super::{command, connection, data_frame, event_frame, start_then_periodic, transport};

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
