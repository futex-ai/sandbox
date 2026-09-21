//! Unit tests for split-stream bounded process collection.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD};
use bytes::Bytes;
use unimock::{MockFn, Unimock, matching};

use crate::error::Result;
use crate::{ProcessConnection, ProcessTransport, SplitProcessCommand};

use crate::process::{
    connect::ConnectProcessTransport,
    framing::encode_frame,
    http::{ByteStream, ConnectHttpTransport, stream as stream_call, unary as unary_call},
};

struct NeverConnectingTransport;

#[async_trait]
impl ConnectHttpTransport for NeverConnectingTransport {
    async fn stream(
        &self,
        _connection: ProcessConnection,
        _method: String,
        _request_json: Vec<u8>,
    ) -> Result<ByteStream> {
        std::future::pending().await
    }

    async fn unary(
        &self,
        _connection: ProcessConnection,
        _method: String,
        _request_json: Vec<u8>,
        _ambiguous: bool,
    ) -> Result<Vec<u8>> {
        unreachable!("the stalled stream never yields a process id")
    }

    async fn download(
        &self,
        _connection: ProcessConnection,
        _path: String,
        _offset: u64,
        _max_bytes: usize,
    ) -> Result<Vec<u8>> {
        unreachable!("file transfer is outside this test")
    }

    async fn upload(
        &self,
        _connection: ProcessConnection,
        _path: String,
        _bytes: Vec<u8>,
    ) -> Result<()> {
        unreachable!("file transfer is outside this test")
    }
}

#[tokio::test]
async fn split_run_separates_streams_and_reports_exit() {
    let events = vec![
        event_frame(r#"{"event":{"start":{"pid":11}}}"#),
        event_frame(&data_frame("stdout", "out-bytes")),
        event_frame(&data_frame("stderr", "Session: bsr_1")),
        event_frame(&data_frame("stdout", "-more")),
        event_frame(r#"{"event":{"end":{"exitCode":0,"exited":true}}}"#),
    ];
    let transport = transport(Unimock::new(
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc(Arc::new(move |_, _, _, _| Ok(byte_stream(events.clone())))),
    ));

    let output = transport
        .run_split(connection(), command(1024, 1024, Duration::from_secs(5)))
        .await
        .expect("split run should decode");

    assert_eq!(output.stdout, b"out-bytes-more");
    assert_eq!(output.stderr, b"Session: bsr_1");
    assert_eq!(output.exit_code, Some(0));
    assert!(output.exited);
    assert!(!output.stdout_overflowed);
    assert!(!output.stderr_overflowed);
}

#[tokio::test]
async fn split_run_reports_overflow_and_kills_the_process() {
    let events = vec![
        event_frame(r#"{"event":{"start":{"pid":19}}}"#),
        event_frame(&data_frame("stdout", "0123456789")),
    ];
    let transport = transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc(Arc::new(move |_, _, _, _| Ok(byte_stream(events.clone())))),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .returns(Ok(Vec::new())),
    )));

    let output = transport
        .run_split(connection(), command(4, 1024, Duration::from_secs(5)))
        .await
        .expect("overflow is reported as data");

    assert_eq!(output.stdout, b"0123");
    assert!(output.stdout_overflowed);
    assert!(!output.stderr_overflowed);
    assert!(!output.exited);
    assert_eq!(output.exit_code, None);
}

#[tokio::test]
async fn signalled_process_end_is_terminal_without_a_second_kill() {
    let events = vec![
        event_frame(r#"{"event":{"start":{"pid":19}}}"#),
        event_frame(r#"{"event":{"end":{"exitCode":-1,"exited":false}}}"#),
    ];
    let transport = transport(Unimock::new(
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc(Arc::new(move |_, _, _, _| Ok(byte_stream(events.clone())))),
    ));
    let output = transport
        .run_split(connection(), command(64, 64, Duration::from_secs(5)))
        .await
        .expect("observed process end");
    assert_eq!(output.exit_code, Some(-1));
    assert!(!output.exited);
}

#[tokio::test]
async fn split_run_decode_failure_after_start_kills_the_process() {
    let events = vec![
        event_frame(r#"{"event":{"start":{"pid":23}}}"#),
        event_frame(r#"{"event":{"data":{"stdout":"not-base64!"}}}"#),
    ];
    let transport = transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc(Arc::new(move |_, _, _, _| Ok(byte_stream(events.clone())))),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .returns(Ok(Vec::new())),
    )));

    assert!(
        transport
            .run_split(connection(), command(1024, 1024, Duration::from_secs(5)))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn coalesced_start_before_malformed_frame_still_kills_observed_pid() {
    let fragment = [
        event_frame(r#"{"event":{"start":{"pid":29}}}"#),
        vec![4, 0, 0, 0, 0],
    ]
    .concat();
    let transport = transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc(Arc::new(move |_, _, _, _| {
                Ok(byte_stream(vec![fragment.clone()]))
            })),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .answers(&|_, _, _, request: Vec<u8>, _| {
                assert_eq!(
                    request,
                    br#"{"process":{"pid":29},"signal":"SIGNAL_SIGKILL"}"#
                );
                Ok(Vec::new())
            }),
    )));

    assert!(
        transport
            .run_split(connection(), command(1024, 1024, Duration::from_secs(5)))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn split_run_deadline_returns_non_exited_output() {
    let transport = transport(Unimock::new(
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers(&|_, _, _, _| Ok(Box::pin(futures_util::stream::pending()))),
    ));

    let output = transport
        .run_split(connection(), command(64, 64, Duration::from_millis(5)))
        .await
        .expect("deadline expiry is a bounded non-exited output");

    assert!(!output.exited);
    assert_eq!(output.exit_code, None);
    assert!(output.stdout.is_empty());
}

#[tokio::test]
async fn split_run_deadline_includes_stream_establishment() {
    let transport = ConnectProcessTransport {
        http: Arc::new(NeverConnectingTransport),
        backend_id: "configured-e2b".to_owned(),
    };

    let output = tokio::time::timeout(
        Duration::from_millis(100),
        transport.run_split(connection(), command(64, 64, Duration::from_millis(5))),
    )
    .await
    .expect("the invocation deadline must bound stream establishment")
    .expect("deadline expiry is a bounded non-exited output");

    assert!(!output.exited);
    assert_eq!(output.exit_code, None);
}

fn command(stdout_limit: usize, stderr_limit: usize, deadline: Duration) -> SplitProcessCommand {
    SplitProcessCommand {
        command: "bowser".to_owned(),
        args: vec!["capture".to_owned()],
        stdout_limit,
        stderr_limit,
        deadline,
    }
}

fn data_frame(channel: &str, value: &str) -> String {
    format!(
        r#"{{"event":{{"data":{{"{channel}":"{}"}}}}}}"#,
        STANDARD.encode(value)
    )
}

fn transport(mock: Unimock) -> ConnectProcessTransport {
    let http: Arc<dyn ConnectHttpTransport> = Arc::new(mock);
    ConnectProcessTransport {
        http,
        backend_id: "configured-e2b".to_owned(),
    }
}

fn connection() -> ProcessConnection {
    ProcessConnection::new(
        "sandbox".to_owned(),
        "e2b.app".to_owned(),
        "access-token".to_owned(),
    )
}

fn event_frame(json: &str) -> Vec<u8> {
    encode_frame(json.as_bytes()).expect("test event frame")
}

fn byte_stream(fragments: Vec<Vec<u8>>) -> ByteStream {
    Box::pin(futures_util::stream::iter(
        fragments
            .into_iter()
            .map(|fragment| Ok(Bytes::from(fragment))),
    ))
}
