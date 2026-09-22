//! Output capture policy tests for non-interactive processes.

use std::{sync::Arc, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD};
use bytes::Bytes;
use futures_util::StreamExt;
use sandbox_interface::Error as DomainError;
use unimock::{MockFn, Unimock, matching};

use crate::process::{
    ConnectProcessTransport, ProcessCommand, ProcessConnection, ProcessOutputCapture,
    ProcessTransport,
    framing::encode_frame,
    http::{ByteStream, ConnectHttpTransport, stream as stream_call, unary as unary_call},
};

#[tokio::test]
async fn tail_capture_keeps_draining_until_exit() {
    let events = vec![
        data_event("1234"),
        data_event("5678"),
        event_frame(r#"{"event":{"end":{"exitCode":9,"exited":true}}}"#),
        success_end_stream_frame(),
    ];
    let transport = process_transport(events);

    let output = transport
        .run(
            connection(),
            command(ProcessOutputCapture::Tail { max_bytes: 5 }),
        )
        .await
        .expect("tail capture should not fail on overflow");

    assert_eq!(output.bytes, b"45678");
    assert_eq!(output.exit_code, Some(9));
    assert!(output.exited);
    assert!(output.output_truncated);
}

#[tokio::test]
async fn exact_limit_tail_capture_is_complete() {
    let events = vec![
        data_event("12345"),
        event_frame(r#"{"event":{"end":{"exitCode":1,"exited":true}}}"#),
        success_end_stream_frame(),
    ];

    let output = process_transport(events)
        .run(
            connection(),
            command(ProcessOutputCapture::Tail { max_bytes: 5 }),
        )
        .await
        .expect("exact-limit output should remain complete");

    assert_eq!(output.bytes, b"12345");
    assert!(!output.output_truncated);
}

#[tokio::test]
async fn hard_limit_capture_still_rejects_overflow() {
    let events = vec![
        event_frame(r#"{"event":{"start":{"pid":31}}}"#),
        data_event("123456"),
    ];
    let mock = Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc(Arc::new(move |_, _, _, _| Ok(byte_stream(events.clone())))),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .answers(&|_, _, _, request: Vec<u8>, _| {
                assert_eq!(
                    request,
                    br#"{"process":{"pid":31},"signal":"SIGNAL_SIGKILL"}"#
                );
                Ok(Vec::new())
            }),
    ));
    let error = transport(mock)
        .run(
            connection(),
            command(ProcessOutputCapture::HardLimit { max_bytes: 5 }),
        )
        .await
        .expect_err("helper overflow must fail closed");

    assert!(matches!(error, DomainError::Internal(_)));
}

#[tokio::test]
async fn helper_deadline_after_start_kills_the_observed_process() {
    let started = event_frame(r#"{"event":{"start":{"pid":37}}}"#);
    let mock = Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc(Arc::new(move |_, _, _, _| {
                let first = futures_util::stream::iter([Ok(Bytes::from(started.clone()))]);
                Ok(Box::pin(first.chain(futures_util::stream::pending())))
            })),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .returns(Ok(Vec::new())),
    ));

    let error = transport(mock)
        .run_helper(
            connection(),
            command(ProcessOutputCapture::HardLimit { max_bytes: 5 }),
            Duration::from_millis(5),
        )
        .await
        .expect_err("an unfinished helper must fail after cleanup");

    assert!(matches!(error, DomainError::BackendUnavailable { .. }));
}

#[tokio::test]
async fn helper_rejects_a_non_normal_end_even_when_its_default_code_is_zero() {
    let events = vec![
        event_frame(r#"{"event":{"end":{"exitCode":0,"exited":false}}}"#),
        success_end_stream_frame(),
    ];

    let error = process_transport(events)
        .run_helper(
            connection(),
            command(ProcessOutputCapture::HardLimit { max_bytes: 5 }),
            Duration::from_secs(5),
        )
        .await
        .expect_err("signal termination must not look like helper success");

    assert!(matches!(error, DomainError::BackendUnavailable { .. }));
}

fn process_transport(events: Vec<Vec<u8>>) -> ConnectProcessTransport {
    let mock = Unimock::new(
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc(Arc::new(move |_, _, _, _| Ok(byte_stream(events.clone())))),
    );
    transport(mock)
}

fn transport(mock: Unimock) -> ConnectProcessTransport {
    let http: Arc<dyn ConnectHttpTransport> = Arc::new(mock);
    ConnectProcessTransport {
        http,
        backend_id: "configured-e2b".to_owned(),
    }
}

fn command(output_capture: ProcessOutputCapture) -> ProcessCommand {
    ProcessCommand {
        command: "/bin/false".to_owned(),
        args: Vec::new(),
        cwd: None,
        output_capture,
        timeout: Duration::from_secs(10),
        read_only: false,
    }
}

fn connection() -> ProcessConnection {
    ProcessConnection::new(
        "sandbox".to_owned(),
        "e2b.app".to_owned(),
        "access-token".to_owned(),
    )
}

fn data_event(text: &str) -> Vec<u8> {
    event_frame(&format!(
        r#"{{"event":{{"data":{{"stdout":"{}"}}}}}}"#,
        STANDARD.encode(text)
    ))
}

fn event_frame(json: &str) -> Vec<u8> {
    encode_frame(json.as_bytes()).expect("test event frame")
}

fn success_end_stream_frame() -> Vec<u8> {
    let mut frame = event_frame("{}");
    frame[0] = 2;
    frame
}

fn byte_stream(fragments: Vec<Vec<u8>>) -> ByteStream {
    Box::pin(futures_util::stream::iter(
        fragments
            .into_iter()
            .map(|fragment| Ok(Bytes::from(fragment))),
    ))
}
