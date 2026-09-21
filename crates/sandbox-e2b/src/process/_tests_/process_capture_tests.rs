//! Output capture policy tests for non-interactive processes.

use std::{sync::Arc, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD};
use bytes::Bytes;
use sandbox_interface::Error as DomainError;
use unimock::{MockFn, Unimock, matching};

use crate::process::{
    ConnectProcessTransport, ProcessCommand, ProcessConnection, ProcessOutputCapture,
    ProcessTransport,
    framing::encode_frame,
    http::{ByteStream, ConnectHttpTransport, stream as stream_call},
};

#[tokio::test]
async fn tail_capture_keeps_draining_until_exit() {
    let events = vec![
        data_event("1234"),
        data_event("5678"),
        event_frame(r#"{"event":{"end":{"exitCode":9,"exited":true}}}"#),
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
    let error = process_transport(vec![data_event("123456")])
        .run(
            connection(),
            command(ProcessOutputCapture::HardLimit { max_bytes: 5 }),
        )
        .await
        .expect_err("helper overflow must fail closed");

    assert!(matches!(error, DomainError::Internal(_)));
}

fn process_transport(events: Vec<Vec<u8>>) -> ConnectProcessTransport {
    let mock = Unimock::new(
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc(Arc::new(move |_, _, _, _| Ok(byte_stream(events.clone())))),
    );
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

fn byte_stream(fragments: Vec<Vec<u8>>) -> ByteStream {
    Box::pin(futures_util::stream::iter(
        fragments
            .into_iter()
            .map(|fragment| Ok(Bytes::from(fragment))),
    ))
}
