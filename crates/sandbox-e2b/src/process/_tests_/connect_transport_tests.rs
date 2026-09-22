//! Unit tests for the high-level Connect process transport.

use std::{sync::Arc, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD};
use bytes::Bytes;
use sandbox_interface::{Error as DomainError, ResourceKind};
use serde_json::Value;
use unimock::{MockFn, Unimock, matching};

use crate::{
    E2bAdapterError, ProcessCommand, ProcessConnection, ProcessOutputCapture, ProcessPtyRequest,
    ProcessSelector, ProcessTransport,
};

use super::ConnectProcessTransport;
use crate::process::{
    framing::encode_frame,
    http::{
        ByteStream, ConnectHttpTransport, download as download_call, stream as stream_call,
        unary as unary_call,
    },
    wire::{encode, pty_start},
};

impl ConnectProcessTransport {
    fn with_http(backend_id: impl Into<String>, http: Arc<dyn ConnectHttpTransport>) -> Self {
        Self {
            http,
            backend_id: backend_id.into(),
        }
    }
}

#[test]
fn pty_start_caps_only_the_private_provider_log() {
    let body: Value = serde_json::from_slice(
        &encode(&pty_start(ProcessPtyRequest {
            tag: "sandbox-terminal".to_owned(),
            log_path: "/tmp/sandbox.log".to_owned(),
            log_limit: 2 * 1024 * 1024,
            workload_user: "user".to_owned(),
            cwd: None,
        }))
        .expect("PTY body"),
    )
    .expect("PTY JSON");

    assert_eq!(body["process"]["cmd"], "/usr/bin/python3");
    assert_eq!(body["process"]["args"][0], "-I");
    assert_eq!(body["process"]["args"][1], "-S");
    assert_eq!(body["process"]["args"][2], "-c");
    assert_eq!(body["process"]["args"][4], "/tmp/sandbox.log");
    assert_eq!(body["process"]["args"][5], (2 * 1024 * 1024).to_string());
    assert_eq!(body["process"]["args"][6], "user");
    let wrapper = body["process"]["args"][3]
        .as_str()
        .expect("wrapper command");
    assert!(wrapper.contains("os.O_NOFOLLOW"));
    assert!(wrapper.contains("os.dup2"));
    assert!(wrapper.contains("resource.RLIMIT_FSIZE"));
    assert!(wrapper.contains("LOG_DESCRIPTOR = 3"));
    assert!(wrapper.contains("/proc/self/fd/{LOG_DESCRIPTOR}"));
    assert!(wrapper.contains("os.closerange"));
    assert!(wrapper.contains("os.setuid"));
    assert!(wrapper.contains("os.execv('/bin/bash', ['bash', '-il'])"));
    assert!(!wrapper.contains("--log-size"));
    assert!(!wrapper.contains("/dev/null"));
}

#[tokio::test]
async fn fragmented_start_stream_returns_the_provider_pid() {
    let frame = event_frame(r#"{"event":{"start":{"pid":77}}}"#);
    let split = frame.len() - 3;
    let fragments = vec![frame[..split].to_vec(), frame[split..].to_vec()];
    let transport = process_transport(Unimock::new(
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc(Arc::new(move |_, _, _, _| {
                Ok(byte_stream(fragments.clone()))
            })),
    ));

    let process = transport
        .start_pty(
            connection(),
            ProcessPtyRequest {
                tag: "sandbox-terminal".to_owned(),
                log_path: "/tmp/sandbox.log".to_owned(),
                log_limit: 1024,
                workload_user: "user".to_owned(),
                cwd: None,
            },
        )
        .await
        .expect("start should decode");

    assert_eq!(process.pid, 77);
    assert_eq!(process.tag.as_deref(), Some("sandbox-terminal"));
}

#[tokio::test]
async fn connect_decodes_output_exit_and_timeout() {
    let data = format!(
        r#"{{"event":{{"data":{{"pty":"{}"}}}}}}"#,
        STANDARD.encode("hello")
    );
    let fragments = vec![
        event_frame(&data),
        event_frame(r#"{"event":{"end":{"exitCode":3,"exited":true}}}"#),
        success_end_stream_frame(),
    ];
    let completed = process_transport(Unimock::new(
        stream_call
            .next_call(matching!(_, "Connect", _))
            .answers_arc(Arc::new(move |_, _, _, _| {
                Ok(byte_stream(fragments.clone()))
            })),
    ));
    let timed_out = process_transport(Unimock::new(
        stream_call
            .next_call(matching!(_, "Connect", _))
            .answers(&|_, _, _, _| Ok(Box::pin(futures_util::stream::pending()))),
    ));

    let output = completed
        .connect(connection(), 7, Duration::from_secs(1), 32)
        .await
        .expect("connect should decode");
    let timeout = timed_out
        .connect(connection(), 7, Duration::from_millis(1), 32)
        .await
        .expect("timeout is a bounded empty poll");

    assert_eq!(output.bytes, b"hello");
    assert_eq!(output.exit_code, Some(3));
    assert!(output.exited);
    assert!(timeout.bytes.is_empty());
    assert!(!timeout.exited);
}

#[tokio::test]
async fn oversized_stream_and_ambiguous_input_fail_closed() {
    let data = format!(
        r#"{{"event":{{"data":{{"pty":"{}"}}}}}}"#,
        STANDARD.encode("four")
    );
    let oversized = process_transport(Unimock::new(
        stream_call
            .next_call(matching!(_, "Connect", _))
            .answers_arc(Arc::new(move |_, _, _, _| {
                Ok(byte_stream(vec![event_frame(&data)]))
            })),
    ));
    let ambiguous = process_transport(Unimock::new(
        unary_call
            .next_call(matching!(_, "SendInput", _, true))
            .returns(Err(E2bAdapterError::DeliveryAmbiguous)),
    ));

    let output_error = oversized
        .connect(connection(), 7, Duration::from_secs(1), 3)
        .await
        .expect_err("oversized output should fail");
    let input_error = ambiguous
        .send_input(connection(), ProcessSelector::Pid(7), b"input".to_vec())
        .await
        .expect_err("ambiguous input should fail");

    assert!(matches!(output_error, DomainError::Internal(_)));
    assert!(matches!(input_error, DomainError::DeliveryUnknown));
}

#[tokio::test]
async fn missing_file_transfer_is_typed_as_a_file() {
    let transport = process_transport(Unimock::new(
        download_call
            .next_call(matching!(_, "/workspace/repo/gone.txt", 0, 512))
            .returns(Err(E2bAdapterError::NotFound)),
    ));

    let result = transport
        .download_file(connection(), "/workspace/repo/gone.txt".to_owned(), 0, 512)
        .await;

    assert!(matches!(
        result,
        Err(DomainError::NotFound {
            resource: ResourceKind::File
        })
    ));
}

#[tokio::test]
async fn run_and_list_decode_typed_unary_and_stream_responses() {
    let events = vec![
        event_frame(r#"{"event":{"data":{"stdout":"b2s="}}}"#),
        event_frame(r#"{"event":{"end":{"exitCode":0,"exited":true}}}"#),
        success_end_stream_frame(),
    ];
    let transport = process_transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc(Arc::new(move |_, _, _, _| Ok(byte_stream(events.clone())))),
        unary_call
            .next_call(matching!(_, "List", _, false))
            .returns(Ok(
                br#"{"processes":[{"pid":8,"tag":"sandbox-terminal"}]}"#.to_vec()
            )),
    )));

    let run = transport
        .run(
            connection(),
            ProcessCommand {
                command: "/bin/true".to_owned(),
                args: Vec::new(),
                cwd: Some("/workspace".to_owned()),
                output_capture: ProcessOutputCapture::HardLimit { max_bytes: 16 },
                timeout: Duration::from_secs(10),
                read_only: false,
            },
        )
        .await
        .expect("run should decode");
    let listed = transport
        .list(connection())
        .await
        .expect("list should decode");

    assert_eq!(run.bytes, b"ok");
    assert_eq!(run.exit_code, Some(0));
    assert!(!run.output_truncated);
    assert_eq!(listed[0].pid, 8);
}

fn process_transport(mock: Unimock) -> ConnectProcessTransport {
    let http: Arc<dyn ConnectHttpTransport> = Arc::new(mock);
    ConnectProcessTransport::with_http("configured-e2b", http)
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
