//! Connect end-stream application-error regressions.

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use sandbox_interface::Error as DomainError;
use unimock::{MockFn, Unimock, matching};

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
                stdout_limit: 1024,
                stderr_limit: 1024,
                deadline: Duration::from_secs(1),
            },
        )
        .await
        .expect_err("provider end-stream error must fail collection");

    assert!(matches!(error, DomainError::BackendUnavailable { .. }));
}

fn transport() -> ConnectProcessTransport {
    let frame = end_stream_frame(br#"{"error":{"code":"unavailable","message":"failed"}}"#);
    let http: Arc<dyn ConnectHttpTransport> = Arc::new(Unimock::new(
        stream_call
            .next_call(matching!(_, _, _))
            .answers_arc(Arc::new(move |_, _, _, _| {
                Ok(byte_stream(vec![frame.clone()]))
            })),
    ));
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

fn end_stream_frame(payload: &[u8]) -> Vec<u8> {
    let length = u32::try_from(payload.len()).expect("test frame length");
    let mut frame = vec![2];
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
