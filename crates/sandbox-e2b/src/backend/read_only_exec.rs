//! Stateless read-only command execution through E2B envd.

use sandbox_interface::{BackendReadOnlyExecRequest, Error, ReadOnlyExecOutput, Result};

use crate::process::{ProcessCommand, ProcessOutputCapture};

use super::{configured::E2bSandboxBackend, mapping};

pub(super) async fn execute(
    backend: &E2bSandboxBackend,
    request: BackendReadOnlyExecRequest,
) -> Result<ReadOnlyExecOutput> {
    let connection = mapping::read_only_connection(backend, &request.sandbox_provider_ref).await?;
    let output = backend
        .processes
        .run(
            connection,
            ProcessCommand {
                command: request.executable,
                args: request.args,
                cwd: Some(request.cwd),
                output_capture: ProcessOutputCapture::HardLimit {
                    max_bytes: request.output_limit,
                },
                timeout: request.timeout,
                read_only: true,
            },
        )
        .await?;
    if output.output_truncated {
        return Err(Error::ReadOnlyOutputTooLarge);
    }
    let Some(exit_code) = output.exit_code else {
        return Err(Error::BackendUnavailable {
            backend_id: backend.config.backend_id.clone(),
        });
    };
    if !output.exited {
        return Err(Error::BackendUnavailable {
            backend_id: backend.config.backend_id.clone(),
        });
    }
    Ok(ReadOnlyExecOutput {
        bytes: output.bytes,
        exit_code,
    })
}
