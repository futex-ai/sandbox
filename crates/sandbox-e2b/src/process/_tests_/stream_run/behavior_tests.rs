//! Event ordering, overflow, and trailer regressions.

use std::{sync::Arc, time::Duration};

use futures_util::StreamExt;
use sandbox_interface::{ProcessStreamEvent, ProcessStreamOutcome};
use unimock::{MockFn, Unimock, matching};

use crate::ProcessTransport;
use crate::process::http::{stream_with_timeout as stream_call, unary as unary_call};

use super::{
    byte_stream, command, connection, data_frame, event_frame, success_trailer, transport,
};

#[tokio::test]
async fn stream_preserves_event_order_and_waits_for_success_trailer() {
    let events = vec![
        event_frame(r#"{"event":{"start":{"pid":11}}}"#),
        data_frame("stdout", "out"),
        data_frame("stderr", "err"),
        event_frame(r#"{"event":{"end":{"exitCode":7,"exited":true}}}"#),
        success_trailer(),
    ];
    let transport = transport(Unimock::new(
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc(Arc::new(move |_, _, _, _, request_timeout| {
                assert_eq!(request_timeout, Duration::from_secs(3610));
                Ok(byte_stream(events.clone()))
            })),
    ));

    let stream = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_secs(3600), Duration::from_secs(2)),
        )
        .await
        .expect("stream should start");

    assert_eq!(
        stream.collect::<Vec<_>>().await,
        [
            ProcessStreamEvent::Started { pid: 11 },
            ProcessStreamEvent::Stdout(b"out".to_vec()),
            ProcessStreamEvent::Stderr(b"err".to_vec()),
            ProcessStreamEvent::Exited {
                exit_code: 7,
                exited: true,
            },
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::Completed),
        ]
    );
}

#[tokio::test]
async fn stream_preserves_signal_termination_with_a_default_zero_exit_code() {
    let events = vec![
        event_frame(r#"{"event":{"start":{"pid":17}}}"#),
        event_frame(r#"{"event":{"end":{"exitCode":0,"exited":false}}}"#),
        success_trailer(),
    ];
    let transport = transport(Unimock::new(
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc(Arc::new(move |_, _, _, _, _| {
                Ok(byte_stream(events.clone()))
            })),
    ));

    let stream = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_secs(5), Duration::from_secs(2)),
        )
        .await
        .expect("stream should start");

    assert_eq!(
        stream.collect::<Vec<_>>().await,
        [
            ProcessStreamEvent::Started { pid: 17 },
            ProcessStreamEvent::Exited {
                exit_code: 0,
                exited: false,
            },
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::Completed),
        ]
    );
}

#[tokio::test]
async fn stream_emits_bounded_prefix_then_stderr_overflow_and_kills() {
    let events = vec![
        event_frame(r#"{"event":{"start":{"pid":21}}}"#),
        data_frame("stderr", "abcdef"),
    ];
    let transport = transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc(Arc::new(move |_, _, _, _, _| {
                Ok(byte_stream(events.clone()))
            })),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .returns(Ok(Vec::new())),
    )));

    let stream = transport
        .stream_process(
            connection(),
            command(64, 3, Duration::from_secs(5), Duration::from_secs(2)),
        )
        .await
        .expect("stream should start");

    assert_eq!(
        stream.collect::<Vec<_>>().await,
        [
            ProcessStreamEvent::Started { pid: 21 },
            ProcessStreamEvent::Stderr(b"abc".to_vec()),
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::StderrOverflow),
        ]
    );
}

#[tokio::test]
async fn stream_emits_bounded_prefix_then_stdout_overflow_and_kills() {
    let events = vec![
        event_frame(r#"{"event":{"start":{"pid":19}}}"#),
        data_frame("stdout", "012345"),
    ];
    let transport = transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc(Arc::new(move |_, _, _, _, _| {
                Ok(byte_stream(events.clone()))
            })),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .returns(Ok(Vec::new())),
    )));

    let stream = transport
        .stream_process(
            connection(),
            command(4, 64, Duration::from_secs(5), Duration::from_secs(2)),
        )
        .await
        .expect("stream should start");

    assert_eq!(
        stream.collect::<Vec<_>>().await,
        [
            ProcessStreamEvent::Started { pid: 19 },
            ProcessStreamEvent::Stdout(b"0123".to_vec()),
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::StdoutOverflow),
        ]
    );
}

#[tokio::test]
async fn missing_trailer_is_a_terminal_transport_failure_without_second_kill() {
    let events = vec![
        event_frame(r#"{"event":{"start":{"pid":23}}}"#),
        event_frame(r#"{"event":{"end":{"exitCode":0,"exited":true}}}"#),
    ];
    let transport = transport(Unimock::new(
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc(Arc::new(move |_, _, _, _, _| {
                Ok(byte_stream(events.clone()))
            })),
    ));

    let stream = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_secs(5), Duration::from_secs(2)),
        )
        .await
        .expect("stream should start");

    assert_eq!(
        stream.collect::<Vec<_>>().await,
        [
            ProcessStreamEvent::Started { pid: 23 },
            ProcessStreamEvent::Exited {
                exit_code: 0,
                exited: true,
            },
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::TransportFailure),
        ]
    );
}
