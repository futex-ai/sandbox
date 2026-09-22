//! Cleanup-policy routing for failed replacement writes.

use std::sync::{Arc, Mutex};

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
async fn upload_failure_uses_disposable_cleanup() {
    let cleanup = Arc::new(Mutex::new(None));
    let http = Unimock::new((
        upload_call
            .next_call(matching!(_, _, _))
            .returns(Err(E2bAdapterError::Unavailable)),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc({
                let cleanup = cleanup.clone();
                Arc::new(move |_, _, _, request| {
                    *cleanup.lock().expect("cleanup request lock") = Some(request);
                    Ok(completed_stream(0))
                })
            }),
    ));
    let transport = transport(http);

    let result = write(&transport, connection(), request()).await;

    assert!(matches!(result, Err(Error::BackendUnavailable { .. })));
    assert_cleanup_policy(&cleanup, false);
}

#[tokio::test]
async fn rejected_writer_uses_disposable_cleanup() {
    let cleanup = Arc::new(Mutex::new(None));
    let http = Unimock::new((
        upload_call.next_call(matching!(_, _, _)).returns(Ok(())),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers(&|_, _, _, _| Ok(completed_stream(45))),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc({
                let cleanup = cleanup.clone();
                Arc::new(move |_, _, _, request| {
                    *cleanup.lock().expect("cleanup request lock") = Some(request);
                    Ok(completed_stream(0))
                })
            }),
    ));
    let transport = transport(http);

    let result = write(&transport, connection(), request()).await;

    assert!(matches!(
        result,
        Err(Error::InvalidFilePath { field: "root" })
    ));
    assert_cleanup_policy(&cleanup, false);
}

#[tokio::test]
async fn uncertain_writer_uses_retained_cleanup() {
    let cleanup = Arc::new(Mutex::new(None));
    let http = Unimock::new((
        upload_call.next_call(matching!(_, _, _)).returns(Ok(())),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers(&|_, _, _, _| Err(E2bAdapterError::Unavailable)),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc({
                let cleanup = cleanup.clone();
                Arc::new(move |_, _, _, request| {
                    *cleanup.lock().expect("cleanup request lock") = Some(request);
                    Ok(completed_stream(0))
                })
            }),
    ));
    let transport = transport(http);

    let result = write(&transport, connection(), request()).await;

    assert!(matches!(result, Err(Error::BackendUnavailable { .. })));
    assert_cleanup_policy(&cleanup, true);
}

fn assert_cleanup_policy(cleanup: &Mutex<Option<Vec<u8>>>, retain_fence: bool) {
    let cleanup = cleanup
        .lock()
        .expect("cleanup request lock")
        .clone()
        .expect("cleanup request");
    let cleanup: serde_json::Value = serde_json::from_slice(&cleanup).expect("cleanup JSON");
    let args = cleanup["process"]["args"].as_array().expect("cleanup args");
    assert_eq!(args[args.len() - 2], "workload");
    assert_eq!(
        args.last().expect("cleanup policy"),
        &retain_fence.to_string()
    );
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
    .with_user("workload")
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
