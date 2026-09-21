//! Atomic descriptor-relative regular-file replacement inside an E2B sandbox.

use std::time::Duration;

use sandbox_interface::{Error, FILE_TRANSFER_MAX_BYTES, Result};
use uuid::Uuid;

use super::{
    connect::ConnectProcessTransport,
    mapping::map_file_result,
    regular_file_cleanup::{self, CleanupOutcome, CleanupRequest},
    types::{ProcessCommand, ProcessConnection, ProcessOutputCapture},
};

const WRITE_TIMEOUT: Duration = Duration::from_secs(300);
const WRITER: &str = include_str!("helpers/regular_file_write.py");

/// One atomic replacement write below a trusted absolute root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessRegularFileWriteRequest {
    /// Absolute directory that contains the destination.
    pub root: String,
    /// Normalized root-relative destination path.
    pub path: String,
    /// Complete replacement contents.
    pub bytes: Vec<u8>,
}

pub(super) async fn write(
    transport: &ConnectProcessTransport,
    connection: ProcessConnection,
    request: ProcessRegularFileWriteRequest,
) -> Result<()> {
    if request.bytes.len() > FILE_TRANSFER_MAX_BYTES {
        return Err(Error::FileTooLarge {
            limit: FILE_TRANSFER_MAX_BYTES,
        });
    }
    let nonce = Uuid::new_v4().simple().to_string();
    let staging_path = format!("/tmp/.sandbox-e2b-stage-{nonce}");
    let temporary_name = format!(".sandbox-e2b-write-{nonce}");
    let state_path = format!("/tmp/sandbox-e2b-write-state-{nonce}");
    let expected_size = request.bytes.len();
    let upload = map_file_result(
        transport
            .http
            .upload(connection.clone(), staging_path.clone(), request.bytes)
            .await,
        &transport.backend_id,
    );
    if let Err(error) = upload {
        let _outcome = regular_file_cleanup::cleanup(
            transport,
            connection,
            cleanup_request(
                request.root,
                request.path,
                staging_path,
                temporary_name,
                state_path,
                expected_size,
            ),
        )
        .await;
        return Err(error);
    }
    let output = transport
        .run_helper(
            connection.clone(),
            command(
                request.root.clone(),
                request.path.clone(),
                staging_path.clone(),
                temporary_name.clone(),
                state_path.clone(),
                expected_size,
            ),
            WRITE_TIMEOUT,
        )
        .await;
    match output {
        Ok(output) => match writer_outcome(transport, output.exit_code) {
            WriterOutcome::Succeeded => Ok(()),
            WriterOutcome::Rejected(error) => {
                let _outcome = regular_file_cleanup::cleanup(
                    transport,
                    connection,
                    cleanup_request(
                        request.root,
                        request.path,
                        staging_path,
                        temporary_name,
                        state_path,
                        expected_size,
                    ),
                )
                .await;
                Err(error)
            }
            WriterOutcome::Uncertain(error) => {
                reconcile_failure(
                    transport,
                    connection,
                    cleanup_request(
                        request.root,
                        request.path,
                        staging_path,
                        temporary_name,
                        state_path,
                        expected_size,
                    ),
                    error,
                )
                .await
            }
        },
        Err(error) => {
            reconcile_failure(
                transport,
                connection,
                cleanup_request(
                    request.root,
                    request.path,
                    staging_path,
                    temporary_name,
                    state_path,
                    expected_size,
                ),
                error,
            )
            .await
        }
    }
}

async fn reconcile_failure(
    transport: &ConnectProcessTransport,
    connection: ProcessConnection,
    request: CleanupRequest,
    error: Error,
) -> Result<()> {
    match regular_file_cleanup::cleanup(transport, connection, request).await {
        CleanupOutcome::Committed => Ok(()),
        CleanupOutcome::Revoked => Err(error),
        CleanupOutcome::Unconfirmed => Err(Error::FileWriteUnconfirmed),
    }
}

fn cleanup_request(
    root: String,
    path: String,
    staging_path: String,
    temporary_name: String,
    state_path: String,
    expected_size: usize,
) -> CleanupRequest {
    CleanupRequest {
        root,
        path,
        staging_path,
        temporary_name,
        state_path,
        expected_size,
    }
}

/// Separates exits that precede the commit claim from exits that may follow it.
fn writer_outcome(transport: &ConnectProcessTransport, exit_code: Option<i32>) -> WriterOutcome {
    match exit_code {
        Some(0) => WriterOutcome::Succeeded,
        Some(45) => WriterOutcome::Rejected(Error::InvalidFilePath { field: "root" }),
        Some(46) => WriterOutcome::Rejected(Error::FileNotRegular),
        Some(47) => WriterOutcome::Rejected(Error::FileTooLarge {
            limit: FILE_TRANSFER_MAX_BYTES,
        }),
        Some(48) => WriterOutcome::Rejected(Error::InvalidFilePath { field: "path" }),
        Some(50) => WriterOutcome::Rejected(Error::BackendUnavailable {
            backend_id: transport.backend_id.clone(),
        }),
        _ => WriterOutcome::Uncertain(Error::BackendUnavailable {
            backend_id: transport.backend_id.clone(),
        }),
    }
}

enum WriterOutcome {
    Succeeded,
    Rejected(Error),
    Uncertain(Error),
}

fn command(
    root: String,
    path: String,
    staging_path: String,
    temporary_name: String,
    state_path: String,
    expected_size: usize,
) -> ProcessCommand {
    ProcessCommand {
        command: "/usr/bin/python3".to_owned(),
        args: vec![
            "-c".to_owned(),
            WRITER.to_owned(),
            root,
            path,
            staging_path,
            temporary_name,
            state_path,
            expected_size.to_string(),
            FILE_TRANSFER_MAX_BYTES.to_string(),
        ],
        cwd: None,
        output_capture: ProcessOutputCapture::HardLimit { max_bytes: 4096 },
        timeout: WRITE_TIMEOUT,
        read_only: false,
    }
}

#[cfg(test)]
#[path = "_tests_/regular_file_write_tests.rs"]
mod regular_file_write_tests;

#[cfg(test)]
#[path = "_tests_/regular_file_write_outcome_tests.rs"]
mod regular_file_write_outcome_tests;
