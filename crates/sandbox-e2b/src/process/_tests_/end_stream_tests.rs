//! Connect end-stream application-error regressions.

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use futures_util::{StreamExt, stream};
use sandbox_interface::Error as DomainError;
use unimock::{MockFn, Unimock, matching};

use crate::error::Error;

use super::{
    ConnectProcessTransport, ProcessConnection, ProcessTransport, SplitProcessCommand,
    http::{ByteStream, ConnectHttpTransport, stream as stream_call},
};

#[tokio::test]
async fn combined_collector_rejects_an_error_end_stream_envelope() {
    let transport = transport();

    let error = transport
        .connect(connection(), 7, Duration::from_secs(1), 1024)
        .await
        .expect_err("provider end-stream error must fail collection");

    assert!(matches!(error, DomainError::BackendUnavailable { .. }));
}

#[tokio::test]
async fn split_collector_rejects_an_error_end_stream_envelope() {
    let transport = transport();

    let error = transport
        .run_split(
            connection(),
            SplitProcessCommand {
                command: "/bin/true".to_owned(),
                args: Vec::new(),
                cwd: None,
                envs: Default::default(),
                stdout_limit: 1024,
                stderr_limit: 1024,
                deadline: Duration::from_secs(1),
            },
        )
        .await
        .expect_err("provider end-stream error must fail collection");

    assert!(matches!(error, DomainError::BackendUnavailable { .. }));
}

#[tokio::test]
async fn combined_collector_rejects_an_error_trailer_after_process_end() {
    let transport = transport_with_frames(vec![process_end_frame(), error_end_stream_frame()]);

    let error = transport
        .connect(connection(), 7, Duration::from_secs(1), 1024)
        .await
        .expect_err("provider error after process end must fail collection");

    assert!(matches!(error, DomainError::BackendUnavailable { .. }));
}

#[tokio::test]
async fn split_collector_rejects_an_error_trailer_after_process_end() {
    let transport = transport_with_frames(vec![process_end_frame(), error_end_stream_frame()]);

    let error = transport
        .run_split(connection(), split_command())
        .await
        .expect_err("provider error after process end must fail collection");

    assert!(matches!(error, DomainError::BackendUnavailable { .. }));
}

#[tokio::test]
async fn combined_collector_rejects_a_missing_trailer_after_process_end() {
    let transport = transport_with_frames(vec![process_end_frame()]);

    transport
        .connect(connection(), 7, Duration::from_secs(1), 1024)
        .await
        .expect_err("process end without a final trailer must fail collection");
}

#[tokio::test]
async fn split_collector_rejects_a_missing_trailer_after_process_end() {
    let transport = transport_with_frames(vec![process_end_frame()]);

    transport
        .run_split(connection(), split_command())
        .await
        .expect_err("process end without a final trailer must fail collection");
}

#[tokio::test]
async fn collectors_reject_a_frame_after_the_success_trailer() {
    for split in [false, true] {
        let fragment = [
            process_end_frame(),
            success_end_stream_frame(),
            keepalive_frame(),
        ]
        .concat();
        let transport = transport_with_frames(vec![fragment]);
        let result = if split {
            transport
                .run_split(connection(), split_command())
                .await
                .map(|_| ())
        } else {
            transport
                .connect(connection(), 7, Duration::from_secs(1), 1024)
                .await
                .map(|_| ())
        };

        assert!(result.is_err());
    }
}

#[tokio::test]
async fn collectors_reject_later_http_chunks_after_the_success_trailer() {
    for split in [false, true] {
        for trailing in [keepalive_frame(), vec![0]] {
            let transport = transport_with_frames(vec![
                process_end_frame(),
                success_end_stream_frame(),
                trailing,
            ]);
            let result = if split {
                transport
                    .run_split(connection(), split_command())
                    .await
                    .map(|_| ())
            } else {
                transport
                    .connect(connection(), 7, Duration::from_secs(1), 1024)
                    .await
                    .map(|_| ())
            };

            assert!(result.is_err(), "a later chunk must invalidate completion");
        }
    }
}

#[tokio::test(start_paused = true)]
async fn collectors_require_http_eof_even_after_a_success_trailer() {
    for split in [false, true] {
        for stall in [false, true] {
            let http: Arc<dyn ConnectHttpTransport> = Arc::new(Unimock::new(
                stream_call
                    .next_call(matching!(_, _, _))
                    .answers_arc(Arc::new(move |_, _, _, _| {
                        let tail: ByteStream = if stall {
                            Box::pin(stream::pending())
                        } else {
                            Box::pin(stream::iter([Err(Error::Unavailable)]))
                        };
                        let frames = vec![process_end_frame(), success_end_stream_frame()];
                        Ok(Box::pin(byte_stream(frames).chain(tail)))
                    })),
            ));
            let transport = ConnectProcessTransport {
                http,
                backend_id: "configured-e2b".to_owned(),
            };
            let result = if split {
                transport
                    .run_split(connection(), split_command())
                    .await
                    .map(|_| ())
            } else {
                transport
                    .connect(connection(), 7, Duration::from_secs(1), 1024)
                    .await
                    .map(|_| ())
            };

            assert!(result.is_err(), "a trailer cannot replace HTTP EOF");
        }
    }
}

fn transport() -> ConnectProcessTransport {
    transport_with_frames(vec![error_end_stream_frame()])
}

fn transport_with_frames(frames: Vec<Vec<u8>>) -> ConnectProcessTransport {
    let http: Arc<dyn ConnectHttpTransport> = Arc::new(Unimock::new(
        stream_call
            .next_call(matching!(_, _, _))
            .answers_arc(Arc::new(move |_, _, _, _| Ok(byte_stream(frames.clone())))),
    ));
    ConnectProcessTransport {
        http,
        backend_id: "configured-e2b".to_owned(),
    }
}

fn split_command() -> SplitProcessCommand {
    SplitProcessCommand {
        command: "/bin/true".to_owned(),
        args: Vec::new(),
        cwd: None,
        envs: Default::default(),
        stdout_limit: 1024,
        stderr_limit: 1024,
        deadline: Duration::from_secs(1),
    }
}

fn process_end_frame() -> Vec<u8> {
    connect_frame(0, br#"{"event":{"end":{"exitCode":0,"exited":true}}}"#)
}

fn error_end_stream_frame() -> Vec<u8> {
    end_stream_frame(br#"{"error":{"code":"unavailable","message":"failed"}}"#)
}

fn success_end_stream_frame() -> Vec<u8> {
    end_stream_frame(b"{}")
}

fn keepalive_frame() -> Vec<u8> {
    connect_frame(0, br#"{"event":{"keepalive":{}}}"#)
}

fn connection() -> ProcessConnection {
    ProcessConnection::new(
        "sandbox".to_owned(),
        "e2b.app".to_owned(),
        "access-token".to_owned(),
    )
}

fn end_stream_frame(payload: &[u8]) -> Vec<u8> {
    connect_frame(2, payload)
}

fn connect_frame(flags: u8, payload: &[u8]) -> Vec<u8> {
    let length = u32::try_from(payload.len()).expect("test frame length");
    let mut frame = vec![flags];
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

fn byte_stream(fragments: Vec<Vec<u8>>) -> ByteStream {
    Box::pin(futures_util::stream::iter(
        fragments
            .into_iter()
            .map(|fragment| Ok(Bytes::from(fragment))),
    ))
}
