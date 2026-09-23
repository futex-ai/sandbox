//! HTTP response finality after a valid Connect trailer.

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use futures_util::{StreamExt, stream};
use sandbox_interface::{ProcessStreamEvent, ProcessStreamOutcome};
use unimock::{MockFn, Unimock, matching};

use crate::ProcessTransport;
use crate::error::Error;
use crate::process::http::stream_with_timeout as stream_call;

use super::support::{byte_stream, command, connection, event_frame, success_trailer, transport};

#[tokio::test]
async fn stream_rejects_later_bytes_or_transport_errors_after_success_trailer() {
    for tail in [
        Some(event_frame(r#"{"event":{"keepalive":{}}}"#)),
        Some(vec![0]),
        None,
    ] {
        let frames = terminal_frames();
        let transport = transport(Unimock::new(
            stream_call
                .next_call(matching!(_, "Start", _, _))
                .answers_arc(Arc::new(move |_, _, _, _, _| {
                    let tail = tail.clone();
                    Ok(Box::pin(byte_stream(frames.clone()).chain(stream::once(
                        async move {
                            tokio::task::yield_now().await;
                            tail.map(Bytes::from).ok_or(Error::Unavailable)
                        },
                    ))))
                })),
        ));

        let events = transport
            .stream_process(
                connection(),
                command(64, 64, Duration::from_secs(5), Duration::from_secs(2)),
            )
            .await
            .expect("accepted stream")
            .collect::<Vec<_>>()
            .await;

        assert_eq!(events, expected(ProcessStreamOutcome::TransportFailure));
    }
}

#[tokio::test(start_paused = true)]
async fn stream_with_a_success_trailer_still_requires_http_eof_within_its_budget() {
    for (deadline, idle) in [(30, 30), (100, 30)] {
        let transport = transport(Unimock::new(
            stream_call
                .next_call(matching!(_, "Start", _, _))
                .answers_arc(Arc::new(move |_, _, _, _, _| {
                    Ok(Box::pin(
                        byte_stream(terminal_frames()).chain(stream::pending()),
                    ))
                })),
        ));
        let events = transport
            .stream_process(
                connection(),
                command(
                    64,
                    64,
                    Duration::from_millis(deadline),
                    Duration::from_millis(idle),
                ),
            )
            .await
            .expect("accepted stream")
            .collect::<Vec<_>>()
            .await;

        assert_eq!(events, expected(ProcessStreamOutcome::TransportFailure));
    }
}

#[tokio::test]
async fn empty_http_chunks_after_trailer_are_allowed_before_eof() {
    let mut frames = terminal_frames();
    frames.extend([Vec::new(), Vec::new()]);
    let transport = transport(Unimock::new(
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc(Arc::new(move |_, _, _, _, _| {
                Ok(byte_stream(frames.clone()))
            })),
    ));
    let events = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_secs(5), Duration::from_secs(2)),
        )
        .await
        .expect("accepted stream")
        .collect::<Vec<_>>()
        .await;

    assert_eq!(events, expected(ProcessStreamOutcome::Completed));
}

fn terminal_frames() -> Vec<Vec<u8>> {
    vec![
        event_frame(r#"{"event":{"start":{"pid":59}}}"#),
        event_frame(r#"{"event":{"end":{"exitCode":0,"exited":true}}}"#),
        success_trailer(),
    ]
}

fn expected(outcome: ProcessStreamOutcome) -> Vec<ProcessStreamEvent> {
    vec![
        ProcessStreamEvent::Started { pid: 59 },
        ProcessStreamEvent::Exited {
            exit_code: 0,
            exited: true,
        },
        ProcessStreamEvent::Outcome(outcome),
    ]
}
