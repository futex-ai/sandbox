//! Atomic descriptor-relative regular-file replacement inside an E2B sandbox.

use std::time::Duration;

use sandbox_interface::{Error, FILE_TRANSFER_MAX_BYTES, Result};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::trusted_python;

use super::{
    connect::ConnectProcessTransport,
    mapping::map_file_result,
    regular_file_cleanup::{self, CleanupOutcome, CleanupRequest},
    types::{ProcessCommand, ProcessConnection, ProcessOutputCapture},
};

const WRITE_TIMEOUT: Duration = Duration::from_secs(300);
const WRITER: &str = include_str!("helpers/regular_file_write.py");
const TRUSTED_PROCESS_USER: &str = "root";
const WRITE_STATE_ROOT: &str = "/var/lib";
const WRITE_STATE_DIRECTORY: &str = "sandbox-e2b/write-fences";

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
    let expected_size = request.bytes.len();
    let expected_digest = payload_digest(&request.bytes);
    let workload_user = connection.user().unwrap_or_default().to_owned();
    let attempt = WriteAttempt {
        root: request.root,
        path: request.path,
        staging_path: format!("/tmp/.sandbox-e2b-stage-{nonce}"),
        temporary_directory: format!(".sandbox-e2b-write-{nonce}"),
        state_root: WRITE_STATE_ROOT.to_owned(),
        state_path: format!("{WRITE_STATE_DIRECTORY}/{nonce}"),
        expected_size,
        expected_digest,
        workload_user,
    };
    let upload = map_file_result(
        transport
            .http
            .upload(
                connection.clone(),
                attempt.staging_path.clone(),
                request.bytes,
            )
            .await,
        &transport.backend_id,
    );
    if let Err(error) = upload {
        let _outcome =
            regular_file_cleanup::cleanup(transport, connection, attempt.cleanup_request()).await;
        return Err(error);
    }
    let output = transport
        .run_helper(
            connection.clone().with_user(TRUSTED_PROCESS_USER),
            command(&attempt),
            WRITE_TIMEOUT,
        )
        .await;
    match output {
        Ok(output) => match writer_outcome(transport, output.exit_code) {
            WriterOutcome::Succeeded => Ok(()),
            WriterOutcome::Rejected(error) => {
                let _outcome =
                    regular_file_cleanup::cleanup(transport, connection, attempt.cleanup_request())
                        .await;
                Err(error)
            }
            WriterOutcome::Uncertain(error) => {
                reconcile_failure(transport, connection, attempt.cleanup_request(), error).await
            }
        },
        Err(error) => {
            reconcile_failure(transport, connection, attempt.cleanup_request(), error).await
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

/// Shared immutable identity for one writer and every reconciliation attempt.
#[derive(Clone)]
struct WriteAttempt {
    root: String,
    path: String,
    staging_path: String,
    temporary_directory: String,
    state_root: String,
    state_path: String,
    expected_size: usize,
    expected_digest: String,
    workload_user: String,
}

impl WriteAttempt {
    fn cleanup_request(&self) -> CleanupRequest {
        CleanupRequest {
            root: self.root.clone(),
            path: self.path.clone(),
            staging_path: self.staging_path.clone(),
            temporary_directory: self.temporary_directory.clone(),
            state_root: self.state_root.clone(),
            state_path: self.state_path.clone(),
            expected_size: self.expected_size,
            expected_digest: self.expected_digest.clone(),
        }
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
        Some(51) => WriterOutcome::Rejected(Error::BackendUnavailable {
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

fn command(attempt: &WriteAttempt) -> ProcessCommand {
    ProcessCommand {
        command: trusted_python::EXECUTABLE.to_owned(),
        args: trusted_python::command_args(
            WRITER,
            [
                attempt.root.clone(),
                attempt.path.clone(),
                attempt.staging_path.clone(),
                attempt.temporary_directory.clone(),
                attempt.state_root.clone(),
                attempt.state_path.clone(),
                attempt.expected_size.to_string(),
                attempt.expected_digest.clone(),
                attempt.workload_user.clone(),
                FILE_TRANSFER_MAX_BYTES.to_string(),
            ],
        ),
        cwd: None,
        output_capture: ProcessOutputCapture::HardLimit { max_bytes: 4096 },
        timeout: WRITE_TIMEOUT,
        read_only: false,
    }
}

fn payload_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

#[cfg(test)]
#[path = "_tests_/regular_file_write_tests.rs"]
mod regular_file_write_tests;

#[cfg(test)]
#[path = "_tests_/regular_file_writer_helper_tests.rs"]
mod regular_file_writer_helper_tests;

#[cfg(test)]
#[path = "_tests_/regular_file_write_outcome_tests.rs"]
mod regular_file_write_outcome_tests;

#[cfg(test)]
#[path = "_tests_/regular_file_integrity_tests.rs"]
mod regular_file_integrity_tests;
