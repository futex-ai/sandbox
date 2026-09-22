//! Writer exit and cleanup reconciliation regressions.

use std::sync::Arc;

use bytes::Bytes;
use sandbox_interface::Error;
use unimock::{MockFn, Unimock, matching};

use crate::{E2bAdapterError, ProcessConnection};

use super::{ProcessRegularFileWriteRequest, write};
use crate::process::{
    connect::ConnectProcessTransport,
    http::{ByteStream, stream as stream_call, upload as upload_call},
};

#[tokio::test]
async fn nonzero_writer_accepts_a_cleanup_confirmed_commit() {
    let transport = transport(Unimock::new((
        upload_call.next_call(matching!(_, _, _)).returns(Ok(())),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers(&|_, _, _, _| Ok(completed_stream(49))),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers(&|_, _, _, _| Ok(completed_stream(51))),
    )));

    write(&transport, connection(), request())
        .await
        .expect("cleanup proved the replacement committed");
}

#[tokio::test]
async fn nonzero_writer_with_unconfirmed_cleanup_returns_a_fencing_error() {
    let transport = transport(Unimock::new((
        upload_call.next_call(matching!(_, _, _)).returns(Ok(())),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers(&|_, _, _, _| Ok(completed_stream(49))),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers(&|_, _, _, _| Err(E2bAdapterError::Unavailable)),
    )));

    let result = write(&transport, connection(), request()).await;

    assert!(matches!(result, Err(Error::FileWriteUnconfirmed)));
}

#[tokio::test]
async fn definitive_precommit_rejection_preserves_its_typed_error() {
    let transport = transport(Unimock::new((
        upload_call.next_call(matching!(_, _, _)).returns(Ok(())),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers(&|_, _, _, _| Ok(completed_stream(45))),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers(&|_, _, _, _| Err(E2bAdapterError::Unavailable)),
    )));

    let result = write(&transport, connection(), request()).await;

    assert!(matches!(
        result,
        Err(Error::InvalidFilePath { field: "root" })
    ));
}

fn transport(http: Unimock) -> ConnectProcessTransport {
    ConnectProcessTransport {
        http: Arc::new(http),
        backend_id: "e2b".to_owned(),
    }
}

fn request() -> ProcessRegularFileWriteRequest {
    ProcessRegularFileWriteRequest {
        root: "/workspace".to_owned(),
        path: "src/lib.rs".to_owned(),
        bytes: b"replacement".to_vec(),
    }
}

fn connection() -> ProcessConnection {
    ProcessConnection::new(
        "sandbox".to_owned(),
        "e2b.app".to_owned(),
        "access-token".to_owned(),
    )
}

fn completed_stream(exit_code: i32) -> ByteStream {
    let start = frame(br#"{"event":{"start":{"pid":7}}}"#);
    let end = frame(
        format!(r#"{{"event":{{"end":{{"exitCode":{exit_code},"exited":true}}}}}}"#).as_bytes(),
    );
    let trailer = success_end_stream_frame();
    Box::pin(futures_util::stream::iter(vec![
        Ok(Bytes::from(start)),
        Ok(Bytes::from(end)),
        Ok(Bytes::from(trailer)),
    ]))
}

fn success_end_stream_frame() -> Vec<u8> {
    let mut trailer = frame(b"{}");
    trailer[0] = 2;
    trailer
}

fn frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![0];
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}
