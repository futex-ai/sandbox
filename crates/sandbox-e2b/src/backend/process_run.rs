//! Bounded split-stream process execution through the envd transport.

use sandbox_interface::{
    BackendRunProcessRequest, Error, PROCESS_RUN_MAX_ARGV_BYTES, PROCESS_RUN_MAX_DEADLINE,
    PROCESS_RUN_MAX_STREAM_BYTES, Result, SandboxProcessOutput,
};

use crate::process::SplitProcessCommand;

use super::{configured::E2bSandboxBackend, mapping};

pub(super) async fn run(
    backend: &E2bSandboxBackend,
    request: BackendRunProcessRequest,
) -> Result<SandboxProcessOutput> {
    validate(&request)?;
    let connection = mapping::connection(backend, &request.sandbox_provider_ref).await?;
    let output = backend
        .processes
        .run_split(
            connection,
            SplitProcessCommand {
                command: request.command,
                args: request.args,
                cwd: request.cwd,
                envs: request.envs,
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

fn validate(request: &BackendRunProcessRequest) -> Result<()> {
    request.validate_execution_context()?;
    validate_argv("command", &request.command, &request.args)?;
    validate_stream_limit("stdout_limit", request.stdout_limit)?;
    validate_stream_limit("stderr_limit", request.stderr_limit)?;
    validate_duration("deadline", request.deadline)
}

pub(super) fn validate_argv(field: &'static str, command: &str, args: &[String]) -> Result<()> {
    if command.is_empty() {
        return Err(Error::EmptyText { field });
    }
    let mut argv_bytes = command.len();
    if argv_bytes > PROCESS_RUN_MAX_ARGV_BYTES {
        return Err(command_too_large());
    }
    for argument in args {
        let Some(total) = argv_bytes.checked_add(argument.len()) else {
            return Err(command_too_large());
        };
        if total > PROCESS_RUN_MAX_ARGV_BYTES {
            return Err(command_too_large());
        }
        argv_bytes = total;
    }
    Ok(())
}

pub(super) fn validate_duration(field: &'static str, duration: std::time::Duration) -> Result<()> {
    if duration > PROCESS_RUN_MAX_DEADLINE {
        return Err(Error::InvalidSeconds {
            field,
            minimum: 0,
            maximum: PROCESS_RUN_MAX_DEADLINE.as_secs(),
        });
    }
    Ok(())
}

fn command_too_large() -> Error {
    Error::CommandTooLarge {
        limit: PROCESS_RUN_MAX_ARGV_BYTES,
    }
}

pub(super) fn validate_stream_limit(field: &'static str, limit: usize) -> Result<()> {
    if limit > PROCESS_RUN_MAX_STREAM_BYTES {
        return Err(Error::InvalidLength {
            field,
            minimum: 0,
            maximum: PROCESS_RUN_MAX_STREAM_BYTES,
        });
    }
    Ok(())
}
