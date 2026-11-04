//! Validate before access and prepare an immutable direct-argv attempt.

use sandbox_interface::{BackendStreamProcessRequest, Error, ProcessEventStream, Result};
use tokio::time::Instant;

use crate::{
    runtime::configured::BoatTrustedRuntime,
    trusted_runtime::{attempt::Attempt, manifest::Manifest},
};

use super::{
    settings::{self, Settings},
    stream,
};

pub(crate) async fn stream(
    runtime: &BoatTrustedRuntime,
    request: BackendStreamProcessRequest,
    combined: Option<usize>,
) -> Result<ProcessEventStream> {
    let origin = Instant::now();
    settings::validate(&request)?;
    let deadline = origin
        .checked_add(request.deadline)
        .ok_or_else(|| Error::internal_message("Boat process deadline overflow"))?;
    let settings = Settings {
        deadline,
        idle_timeout: request.idle_timeout,
        stdout: request.stdout_limit,
        stderr: request.stderr_limit,
        combined,
    };
    let manifest = Manifest::Process {
        command: request.command,
        args: request.args,
        cwd: request.cwd,
        envs: request.envs,
        timeout_ms: request.deadline.as_millis() as u64,
    };
    let prepare = Attempt::prepare(
        runtime.api.clone(),
        request.sandbox_provider_ref.as_str().to_owned(),
        runtime.config.workload_user().to_owned(),
        manifest,
        deadline,
    );
    let attempt = match tokio::time::timeout_at(deadline, prepare).await {
        Ok(Ok(attempt)) => attempt,
        Ok(Err(_)) => return Err(runtime.unavailable()),
        Err(_) => {
            return Ok(Box::pin(futures_util::stream::iter([
                sandbox_interface::ProcessStreamEvent::Outcome(
                    sandbox_interface::ProcessStreamOutcome::DeadlineExpired,
                ),
            ])));
        }
    };
    Ok(stream::events(attempt, settings))
}
