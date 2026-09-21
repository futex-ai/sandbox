//! Bounded split-stream process execution through the envd transport.

use sandbox_interface::{BackendRunProcessRequest, Result, SandboxProcessOutput};

use crate::process::SplitProcessCommand;

use super::{configured::E2bSandboxBackend, mapping};

pub(super) async fn run(
    backend: &E2bSandboxBackend,
    request: BackendRunProcessRequest,
) -> Result<SandboxProcessOutput> {
    let connection = mapping::connection(backend, &request.sandbox_provider_ref).await?;
    let output = backend
        .processes
        .run_split(
            connection,
            SplitProcessCommand {
                command: request.command,
                args: request.args,
                stdout_limit: request.stdout_limit,
                stderr_limit: request.stderr_limit,
                deadline: request.deadline,
            },
        )
        .await?;
    Ok(SandboxProcessOutput {
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code: output.exit_code,
        exited: output.exited,
        stdout_overflowed: output.stdout_overflowed,
        stderr_overflowed: output.stderr_overflowed,
    })
}
