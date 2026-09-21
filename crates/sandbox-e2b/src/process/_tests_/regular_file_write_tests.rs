//! Atomic regular-file replacement helper coverage.

use std::{
    fs,
    os::unix::fs::symlink,
    path::Path,
    process::Command,
    sync::{Arc, Mutex},
};

use bytes::Bytes;
use sandbox_interface::Error;
use tempfile::tempdir;
use unimock::{MockFn, Unimock, matching};

use crate::{E2bAdapterError, ProcessConnection};

use super::{ProcessRegularFileWriteRequest, command, write};
use crate::process::{
    connect::ConnectProcessTransport,
    http::{ByteStream, stream as stream_call, upload as upload_call},
};

const REPLACEMENT_DIGEST: &str = "95713e9cbdd1dfcb2d4080c2537f418d43ca0da25f0d7d6631f4f7c97b89dc47";

#[test]
fn writer_replaces_a_regular_file_through_directory_descriptors() {
    let root = tempdir().expect("temporary write root");
    let stage = tempdir().expect("temporary staging root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    fs::write(root.path().join("src/lib.rs"), b"old").expect("existing file");
    let staged = stage.path().join("upload");
    fs::write(&staged, b"replacement").expect("staged bytes");

    let output = run(root.path(), "src/lib.rs", &staged, b"replacement".len());

    assert!(
        output.status.success(),
        "helper stderr: {:?}",
        output.stderr
    );
    assert_eq!(
        fs::read(root.path().join("src/lib.rs")).expect("replacement file"),
        b"replacement"
    );
    assert!(!staged.exists());
    assert!(fs::symlink_metadata(staged.with_extension("state")).is_err());
}

#[test]
fn writer_rejects_a_leaf_symlink_without_changing_its_target() {
    let root = tempdir().expect("temporary write root");
    let outside = tempdir().expect("outside directory");
    let stage = tempdir().expect("temporary staging root");
    let outside_file = outside.path().join("outside.txt");
    fs::write(&outside_file, b"protected").expect("outside file");
    symlink(&outside_file, root.path().join("target.txt")).expect("leaf symlink");
    let staged = stage.path().join("upload");
    fs::write(&staged, b"replacement").expect("staged bytes");

    let output = run(root.path(), "target.txt", &staged, b"replacement".len());

    assert_eq!(output.status.code(), Some(46));
    assert_eq!(fs::read(outside_file).expect("outside file"), b"protected");
}

#[test]
fn writer_rejects_a_symlinked_parent_without_writing_outside_root() {
    let root = tempdir().expect("temporary write root");
    let outside = tempdir().expect("outside directory");
    let stage = tempdir().expect("temporary staging root");
    symlink(outside.path(), root.path().join("linked")).expect("parent symlink");
    let staged = stage.path().join("upload");
    fs::write(&staged, b"replacement").expect("staged bytes");

    let output = run(
        root.path(),
        "linked/target.txt",
        &staged,
        b"replacement".len(),
    );

    assert_eq!(output.status.code(), Some(46));
    assert!(!outside.path().join("target.txt").exists());
}

#[test]
fn writer_cannot_replace_after_cleanup_claims_revocation() {
    let root = tempdir().expect("temporary write root");
    let stage = tempdir().expect("temporary staging root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    let target = root.path().join("src/lib.rs");
    let staged = stage.path().join("upload");
    let state = stage.path().join("state");
    fs::write(&target, b"old").expect("old target");
    fs::write(&staged, b"replacement").expect("staged bytes");
    symlink("revoked", &state).expect("revocation claim");

    let output = run_with_state(
        root.path(),
        "src/lib.rs",
        &staged,
        &state,
        b"replacement".len(),
    );

    assert_eq!(output.status.code(), Some(50));
    assert_eq!(fs::read(target).expect("unchanged target"), b"old");
}

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
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers(&|_, _, _, _| Err(E2bAdapterError::Unavailable)),
        stream_call
            .next_call(matching!(_, "Start", _))
            .answers_arc({
                let cleanup_request = cleanup_request.clone();
                Arc::new(move |_, _, _, request| {
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
            .is_some_and(|value| value.contains("sandbox-e2b-write-state-"))
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

fn run(root: &Path, path: &str, staged: &Path, expected_size: usize) -> std::process::Output {
    run_with_state(
        root,
        path,
        staged,
        &staged.with_extension("state"),
        expected_size,
    )
}

fn run_with_state(
    root: &Path,
    path: &str,
    staged: &Path,
    state: &Path,
    expected_size: usize,
) -> std::process::Output {
    let command = command(
        root.to_string_lossy().into_owned(),
        path.to_owned(),
        staged.to_string_lossy().into_owned(),
        ".sandbox-write-test".to_owned(),
        state.to_string_lossy().into_owned(),
        expected_size,
        REPLACEMENT_DIGEST.to_owned(),
    );
    Command::new(command.command)
        .args(command.args)
        .output()
        .expect("run atomic writer")
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
