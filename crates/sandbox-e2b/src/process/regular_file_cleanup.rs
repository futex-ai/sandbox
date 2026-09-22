//! Revocation fencing and reconciliation for uncertain replacement writes.

use std::time::Duration;

use crate::trusted_python;

use super::{
    connect::ConnectProcessTransport,
    types::{ProcessCommand, ProcessConnection, ProcessOutputCapture},
};

const TRUSTED_PROCESS_USER: &str = "root";

const CLEANUP_TIMEOUT: Duration = Duration::from_secs(30);
const COMMITTED_EXIT_CODE: i32 = 51;
const CLEANER: &str = include_str!("helpers/regular_file_cleanup.py");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CleanupOutcome {
    Revoked,
    Committed,
    Unconfirmed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CleanupRequest {
    pub(super) root: String,
    pub(super) path: String,
    pub(super) staging_path: String,
    pub(super) temporary_directory: String,
    pub(super) state_root: String,
    pub(super) state_path: String,
    pub(super) expected_size: usize,
    pub(super) expected_digest: String,
}

pub(super) async fn cleanup(
    transport: &ConnectProcessTransport,
    connection: ProcessConnection,
    request: CleanupRequest,
) -> CleanupOutcome {
    let result = transport
        .run_helper(
            connection.with_user(TRUSTED_PROCESS_USER),
            command(request),
            CLEANUP_TIMEOUT,
        )
        .await;
    let outcome = match result {
        Ok(output) if output.succeeded() => CleanupOutcome::Revoked,
        Ok(output) if output.exit_code == Some(COMMITTED_EXIT_CODE) => CleanupOutcome::Committed,
        Ok(_) | Err(_) => CleanupOutcome::Unconfirmed,
    };
    if outcome == CleanupOutcome::Unconfirmed {
        tracing::debug!(event = "e2b_regular_file_cleanup_unconfirmed");
    }
    outcome
}

fn command(request: CleanupRequest) -> ProcessCommand {
    ProcessCommand {
        command: trusted_python::EXECUTABLE.to_owned(),
        args: trusted_python::command_args(
            CLEANER,
            [
                request.root,
                request.path,
                request.staging_path,
                request.temporary_directory,
                request.state_root,
                request.state_path,
                request.expected_size.to_string(),
                request.expected_digest,
            ],
        ),
        cwd: None,
        output_capture: ProcessOutputCapture::HardLimit { max_bytes: 4096 },
        timeout: CLEANUP_TIMEOUT,
        read_only: false,
    }
}

#[cfg(test)]
#[path = "_tests_/regular_file_cleanup_tests.rs"]
mod regular_file_cleanup_tests;
