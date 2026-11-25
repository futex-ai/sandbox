//! Deadline-aware screen resize with confirmed remote process termination.

use std::time::{Duration, SystemTime};

use sandbox_interface::{BackendResizeScreenStackRequest, Error, Result, ScreenViewportSize};
use serde::Deserialize;

use crate::{
    process::{ProcessSplitOutput, SplitProcessCommand},
    trusted_python,
};

use super::{configured::E2bSandboxBackend, mapping};

const MAX_EXECUTION: Duration = Duration::from_secs(15);
const TERMINATION_ALLOWANCE: Duration = Duration::from_secs(3);

/// Exit status for a failed helper reaped before the budget expires.
///
/// With `exited: true`, status 0 proceeds to acknowledgment validation.
/// Status 3 confirms helper failure, and status 124 confirms timeout cleanup.
/// Both failure codes return `Error::BackendUnavailable`. All other outcomes
/// return `Error::ScreenViewportResizeUnconfirmed`.
///
/// The supervisor runs as `python3`. The interpreter can exit with 0 on
/// success, 1 for an uncaught exception, 2 for a command-line usage error,
/// or 120 for a stdout flush failure at exit. A signal death has no normal
/// exit code, and envd reports `exited: false`. An outside kill can produce
/// that report while the helper keeps running. Shells and wrappers report
/// 126, 127, or `128 + signal number`. The supervisor reserves 124 for timeout
/// cleanup. Status 3 is the lowest code outside these cases, so an unexpected
/// supervisor exit cannot look like a helper failure.
const SUPERVISOR_HELPER_FAILURE: i32 = 3;

/// Exit status after the supervisor kills the helper's process group and
/// reaps the helper on budget expiry or receipt of `SIGTERM`, `SIGINT`,
/// `SIGHUP`, `SIGQUIT`, or `SIGALRM`.
const SUPERVISOR_TIMEOUT_CLEANUP: i32 = 124;

/// The helper runs in its own session and process group so the supervisor
/// can kill the group and reap the helper.
///
/// When the supervisor observes the helper exit before the cutoff, it reaps
/// the helper, leaves the group alone, and exits 0 or 3. This lets processes
/// the template helper keeps running on purpose continue.
///
/// On budget expiry, including a helper exit seen at or after the cutoff, the
/// supervisor kills the group, reaps the helper, and exits 124. It takes the
/// same path if it receives a watched termination signal first.
const RESIZE_SUPERVISOR: &str = r#"
# Keep exit codes in sync with SUPERVISOR_HELPER_FAILURE and
# SUPERVISOR_TIMEOUT_CLEANUP in Rust.
import os
import signal
import subprocess
import sys
import time

watched_signals = {
    signal.SIGTERM, signal.SIGINT, signal.SIGHUP, signal.SIGQUIT,
    signal.SIGALRM, signal.SIGCHLD,
}
signal.pthread_sigmask(signal.SIG_BLOCK, watched_signals)
signal.signal(signal.SIGCHLD, signal.SIG_DFL)
budget = int(sys.argv[1]) / 1000
cutoff = time.monotonic() + budget
helper = subprocess.Popen(
    sys.argv[2:],
    start_new_session=True,
    preexec_fn=lambda: signal.pthread_sigmask(signal.SIG_SETMASK, []),
)

while True:
    remaining = cutoff - time.monotonic()
    if remaining <= 0:
        break
    received = signal.sigtimedwait(watched_signals, min(remaining, 0.05))
    if received is not None and received.si_signo != signal.SIGCHLD:
        break
    observed_exit = os.waitid(
        os.P_PID, helper.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT
    )
    if observed_exit is None:
        continue
    if time.monotonic() >= cutoff:
        break
    status = helper.wait()
    sys.exit(0 if status == 0 else 3)

try:
    os.killpg(helper.pid, signal.SIGKILL)
except ProcessLookupError:
    pass
helper.wait()
sys.exit(124)
"#;

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
        Ok(output) => output,
        Err(_) => return Err(Error::ScreenViewportResizeUnconfirmed),
    };
    match (budget, output.exited, output.exit_code) {
        (_, true, Some(0)) => acknowledge(backend, request.viewport, output),
        (Some(_), true, Some(SUPERVISOR_TIMEOUT_CLEANUP | SUPERVISOR_HELPER_FAILURE))
        | (None, _, Some(_)) => Err(unavailable(backend)),
        _ => Err(Error::ScreenViewportResizeUnconfirmed),
    }
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
    let helper_args = [
        "resize".to_owned(),
        viewport.width().to_string(),
        viewport.height().to_string(),
    ];
    let (command, args, deadline) = if let Some(budget) = budget {
        let execution = budget.min(MAX_EXECUTION);
        let supervisor_args = [
            execution.as_millis().to_string(),
            screen_helper_path.to_owned(),
        ];
        (
            trusted_python::EXECUTABLE,
            trusted_python::command_args(
                RESIZE_SUPERVISOR,
                supervisor_args.into_iter().chain(helper_args),
            ),
            execution + TERMINATION_ALLOWANCE,
        )
    } else {
        (screen_helper_path, Vec::from(helper_args), MAX_EXECUTION)
    };
    SplitProcessCommand {
        command: command.to_owned(),
        args,
        cwd: None,
        envs: Default::default(),
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
    if output.stdout_overflowed || output.stderr_overflowed {
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
