//! Safe failure mapping for user-authored image commands.

use std::time::Duration;

use sandbox_interface::{Error, IMAGE_COMMAND_OUTPUT_MAX_BYTES, ImageCommandFailure, Result};

use crate::process::{ProcessCommand, ProcessConnection, ProcessOutputCapture, ProcessRunOutput};

use super::configured::E2bSandboxBackend;

const HELPER_OUTPUT_LIMIT: usize = 4096;
const IMAGE_COMMAND_TIMEOUT: Duration = Duration::from_secs(300);

pub(super) enum ImagePhase {
    Setup,
    Verify(usize),
    Scrub,
}

pub(super) async fn run_phase(
    backend: &E2bSandboxBackend,
    connection: ProcessConnection,
    command: String,
    phase: ImagePhase,
) -> Result<()> {
    let sensitive_values = [
        connection.sandbox_id().to_owned(),
        connection.access_token().to_owned(),
    ];
    let output = backend
        .processes
        .run(
            connection,
            ProcessCommand {
                command: "/bin/sh".to_owned(),
                args: vec!["-c".to_owned(), command],
                cwd: None,
                output_capture: phase.output_capture(),
                timeout: IMAGE_COMMAND_TIMEOUT,
                read_only: false,
            },
        )
        .await?;
    if output.succeeded() {
        return Ok(());
    }
    Err(phase.error(output, &sensitive_values))
}

impl ImagePhase {
    fn output_capture(&self) -> ProcessOutputCapture {
        match self {
            Self::Setup | Self::Verify(_) => ProcessOutputCapture::Tail {
                max_bytes: IMAGE_COMMAND_OUTPUT_MAX_BYTES,
            },
            Self::Scrub => ProcessOutputCapture::HardLimit {
                max_bytes: HELPER_OUTPUT_LIMIT,
            },
        }
    }

    fn error(self, output: ProcessRunOutput, sensitive_values: &[String]) -> Error {
        match self {
            Self::Setup => Error::ImageSetupFailed {
                command: Some(command_failure(output, sensitive_values)),
                retained_sandbox: None,
            },
            Self::Verify(index) => Error::ImageVerificationFailed {
                index,
                command: Some(command_failure(output, sensitive_values)),
                retained_sandbox: None,
            },
            Self::Scrub => Error::ImageScrubFailed,
        }
    }
}

fn command_failure(output: ProcessRunOutput, sensitive_values: &[String]) -> ImageCommandFailure {
    ImageCommandFailure::from_captured_output(
        &output.bytes,
        output.exit_code,
        output.output_truncated,
        sensitive_values,
    )
}
