//! Atomic regular-file replacement helper coverage.

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
async fn failed_writer_attempts_run_bounded_remote_cleanup() {
    let staged_path = Arc::new(Mutex::new(None));
    let cleanup_request = Arc::new(Mutex::new(None));
    let http = Unimock::new((
        upload_call.next_call(matching!(_, _, _)).answers_arc({
            let staged_path = staged_path.clone();
            Arc::new(move |_, _, path, _| {
                *staged_path.lock().expect("staging path lock") = Some(path);
                Ok(())
            })
        }),
        stream_call.next_call(matching!(_, "Start", _)).answers(
            &|_, connection: ProcessConnection, _, request| {
                assert_eq!(connection.user(), Some("root"));
                let request: serde_json::Value =
                    serde_json::from_slice(&request).expect("writer request JSON");
                let args = request["process"]["args"]
                    .as_array()
                    .expect("writer request args");
                assert!(args.iter().any(|value| value == "workload"));
                let writer = args[3].as_str().expect("writer helper");
                let replace = writer.find("os.replace").expect("atomic replacement");
                let ownership = writer.find("os.fchown").expect("ownership handoff");
                assert!(replace < ownership);
                Err(E2bAdapterError::Unavailable)
            },
        ),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc({
                let cleanup_request = cleanup_request.clone();
                Arc::new(move |_, connection: ProcessConnection, _, request| {
                    assert_eq!(connection.user(), Some("root"));
                    *cleanup_request.lock().expect("cleanup request lock") = Some(request);
                    Ok(completed_stream(0))
                })
            }),
    ));
    let transport = ConnectProcessTransport {
        http: Arc::new(http),
        backend_id: "e2b".to_owned(),
    };

    let result = write(
        &transport,
        connection(),
        ProcessRegularFileWriteRequest {
            root: "/workspace".to_owned(),
            path: "src/lib.rs".to_owned(),
            bytes: b"replacement".to_vec(),
        },
    )
    .await;

    assert!(matches!(result, Err(Error::BackendUnavailable { .. })));
    let staged_path = staged_path
        .lock()
        .expect("staging path lock")
        .clone()
        .expect("uploaded staging path");
    let cleanup = cleanup_request
        .lock()
        .expect("cleanup request lock")
        .clone()
        .expect("cleanup helper request");
    let cleanup: serde_json::Value = serde_json::from_slice(&cleanup).expect("cleanup JSON");
    let args = cleanup["process"]["args"].as_array().expect("cleanup args");
    assert!(args.iter().any(|value| value == &staged_path));
    assert!(args.iter().any(|value| value == "/workspace"));
    assert!(args.iter().any(|value| value == "src/lib.rs"));
    assert!(args.iter().any(|value| {
        value
            .as_str()
            .is_some_and(|value| value.starts_with("sandbox-e2b/write-fences/"))
    }));
}

#[tokio::test]
async fn uncertain_writer_with_unconfirmed_revocation_returns_a_fencing_error() {
    let http = Unimock::new((
        upload_call.next_call(matching!(_, _, _)).returns(Ok(())),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers(&|_, _, _, _| Err(E2bAdapterError::Unavailable)),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers(&|_, _, _, _| Err(E2bAdapterError::Unavailable)),
    ));
    let transport = ConnectProcessTransport {
        http: Arc::new(http),
        backend_id: "e2b".to_owned(),
    };

    let result = write(&transport, connection(), request()).await;

    assert!(matches!(result, Err(Error::FileWriteUnconfirmed)));
}

#[tokio::test]
async fn uncertain_transport_accepts_a_durably_committed_write() {
    let http = Unimock::new((
        upload_call.next_call(matching!(_, _, _)).returns(Ok(())),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers(&|_, _, _, _| Err(E2bAdapterError::Unavailable)),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers(&|_, _, _, _| Ok(completed_stream(51))),
    ));
    let transport = ConnectProcessTransport {
        http: Arc::new(http),
        backend_id: "e2b".to_owned(),
    };

    write(&transport, connection(), request())
        .await
        .expect("commit claim proves the replacement completed");
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
    Box::pin(futures_util::stream::iter(vec![
        Ok(Bytes::from(start)),
        Ok(Bytes::from(end)),
    ]))
}

fn frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![0];
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}
