//! Deadline-aware screen resize with confirmed remote process termination.

use std::time::{Duration, SystemTime};

use sandbox_interface::{BackendResizeScreenStackRequest, Error, Result, ScreenViewportSize};
use serde::Deserialize;

use crate::process::{ProcessSplitOutput, SplitProcessCommand};

use super::{configured::E2bSandboxBackend, mapping};

const MAX_EXECUTION: Duration = Duration::from_secs(15);
const TERMINATION_ALLOWANCE: Duration = Duration::from_secs(3);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResizeAcknowledgment {
    version: u32,
    width: u32,
    height: u32,
}

pub(super) async fn resize(
    backend: &E2bSandboxBackend,
    request: BackendResizeScreenStackRequest,
) -> Result<ScreenViewportSize> {
    let deadline = request.deadline_at.map(SystemTime::from);
    let connection = if let Some(deadline) = deadline {
        let budget = remaining(deadline).ok_or_else(|| unavailable(backend))?;
        match tokio::time::timeout(
            budget,
            mapping::read_only_connection(backend, &request.sandbox_provider_ref),
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => return Err(unavailable(backend)),
        }
    } else {
        mapping::connection(backend, &request.sandbox_provider_ref).await?
    };
    let budget = match deadline {
        Some(deadline) => Some(remaining(deadline).ok_or_else(|| unavailable(backend))?),
        None => None,
    };
    let output = match backend
        .processes
        .run_split(
            connection,
            command(
                request.viewport,
                budget,
                backend.config.runtime_conventions().screen_helper_path(),
            ),
        )
        .await
    {
        Ok(output) if output.exit_code.is_some() => output,
        Ok(_) | Err(_) => return Err(Error::ScreenViewportResizeUnconfirmed),
    };
    acknowledge(backend, request.viewport, output)
}

fn remaining(deadline: SystemTime) -> Option<Duration> {
    deadline
        .duration_since(SystemTime::now())
        .ok()?
        .checked_sub(TERMINATION_ALLOWANCE)
        .filter(|remaining| *remaining >= Duration::from_millis(1))
}

fn command(
    viewport: ScreenViewportSize,
    budget: Option<Duration>,
    screen_helper_path: &str,
) -> SplitProcessCommand {
    let mut args = vec![
        "resize".to_owned(),
        viewport.width().to_string(),
        viewport.height().to_string(),
    ];
    let (command, deadline) = if let Some(budget) = budget {
        let execution = budget.min(MAX_EXECUTION);
        let mut supervised = vec![
            "--signal=KILL".to_owned(),
            format!("{}.{:03}s", execution.as_secs(), execution.subsec_millis()),
            screen_helper_path.to_owned(),
        ];
        supervised.append(&mut args);
        args = supervised;
        ("/usr/bin/timeout", execution + TERMINATION_ALLOWANCE)
    } else {
        (screen_helper_path, MAX_EXECUTION)
    };
    SplitProcessCommand {
        command: command.to_owned(),
        args,
        stdout_limit: 4096,
        stderr_limit: 64 * 1024,
        deadline,
    }
}

fn acknowledge(
    backend: &E2bSandboxBackend,
    viewport: ScreenViewportSize,
    output: ProcessSplitOutput,
) -> Result<ScreenViewportSize> {
    if !output.exited
        || output.exit_code != Some(0)
        || output.stdout_overflowed
        || output.stderr_overflowed
    {
        return Err(unavailable(backend));
    }
    let Ok(acknowledgment) = serde_json::from_slice::<ResizeAcknowledgment>(&output.stdout) else {
        return Err(unavailable(backend));
    };
    if acknowledgment.version != 1
        || acknowledgment.width != viewport.width()
        || acknowledgment.height != viewport.height()
    {
        return Err(unavailable(backend));
    }
    Ok(viewport)
}

fn unavailable(backend: &E2bSandboxBackend) -> Error {
    Error::BackendUnavailable {
        backend_id: backend.config.backend_id().to_owned(),
    }
}
