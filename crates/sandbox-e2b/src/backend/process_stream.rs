//! Bounded streaming process execution through the envd transport.

use std::time::Duration;

use sandbox_interface::{
    BackendStreamProcessRequest, Error, PROCESS_STREAM_MAX_DEADLINE, ProcessEventStream, Result,
};

use crate::process::StreamProcessCommand;

use super::{configured::E2bSandboxBackend, mapping, process_run};

pub(super) async fn stream(
    backend: &E2bSandboxBackend,
    request: BackendStreamProcessRequest,
) -> Result<ProcessEventStream> {
    validate(&request)?;
    let timeout_seconds =
        sandbox_timeout_seconds(backend.config.idle_timeout_seconds(), request.deadline)?;
    let connection =
        mapping::connection_with_timeout(backend, &request.sandbox_provider_ref, timeout_seconds)
            .await?;
    backend
        .processes
        .stream_process(
            connection,
            StreamProcessCommand {
                command: request.command,
                args: request.args,
                stdout_limit: request.stdout_limit,
                stderr_limit: request.stderr_limit,
                deadline: request.deadline,
                idle_timeout: request.idle_timeout,
            },
        )
        .await
}

fn validate(request: &BackendStreamProcessRequest) -> Result<()> {
    process_run::validate_argv("command", &request.command, &request.args)?;
    process_run::validate_stream_limit("stdout_limit", request.stdout_limit)?;
    process_run::validate_stream_limit("stderr_limit", request.stderr_limit)?;
    if request.deadline > PROCESS_STREAM_MAX_DEADLINE {
        return Err(Error::InvalidSeconds {
            field: "deadline",
            minimum: 0,
            maximum: PROCESS_STREAM_MAX_DEADLINE.as_secs(),
        });
    }
    if request.idle_timeout.is_zero() || request.idle_timeout > request.deadline {
        return Err(Error::InvalidProcessIdleTimeout);
    }
    Ok(())
}

fn sandbox_timeout_seconds(configured: u32, deadline: Duration) -> Result<u32> {
    let rounded_deadline = deadline
        .as_secs()
        .saturating_add(u64::from(deadline.subsec_nanos() != 0));
    let requested = match u32::try_from(rounded_deadline) {
        Ok(requested) => requested,
        Err(source) => {
            return Err(Error::internal_with(
                source,
                "convert stream deadline to E2B sandbox timeout",
            ));
        }
    };
    Ok(configured.max(requested))
}
