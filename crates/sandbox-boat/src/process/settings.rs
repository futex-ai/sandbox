//! Validated process budgets and independent output accounting.

use std::time::Duration;

use sandbox_interface::{
    BackendStreamProcessRequest, Error, PROCESS_RUN_MAX_ARGV_BYTES, PROCESS_RUN_MAX_STREAM_BYTES,
    PROCESS_STREAM_MAX_DEADLINE, ProcessStreamEvent, ProcessStreamOutcome, Result,
};
use tokio::time::Instant;

pub(crate) fn validate_common(request: &BackendStreamProcessRequest) -> Result<()> {
    request.validate_execution_context()?;
    if request.command.is_empty() {
        return Err(Error::EmptyText { field: "command" });
    }
    let size = request
        .args
        .iter()
        .try_fold(request.command.len(), |n, arg| n.checked_add(arg.len()));
    if size.is_none_or(|n| n > PROCESS_RUN_MAX_ARGV_BYTES) {
        return Err(Error::CommandTooLarge {
            limit: PROCESS_RUN_MAX_ARGV_BYTES,
        });
    }
    for (field, value) in [
        ("stdout_limit", request.stdout_limit),
        ("stderr_limit", request.stderr_limit),
    ] {
        if value > PROCESS_RUN_MAX_STREAM_BYTES {
            return Err(Error::InvalidLength {
                field,
                minimum: 0,
                maximum: PROCESS_RUN_MAX_STREAM_BYTES,
            });
        }
    }
    Ok(())
}

pub(crate) fn validate(request: &BackendStreamProcessRequest) -> Result<()> {
    validate_common(request)?;
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

pub(super) struct Settings {
    pub(super) deadline: Instant,
    pub(super) idle_timeout: Duration,
    pub(super) stdout: usize,
    pub(super) stderr: usize,
    pub(super) combined: Option<usize>,
}

#[derive(Clone, Copy, Default)]
pub(super) struct Observation {
    pub(super) pid: Option<u32>,
    pub(super) ended: bool,
}

pub(super) struct Counts {
    stdout: usize,
    stderr: usize,
    combined: usize,
}
impl Counts {
    pub(super) fn new() -> Self {
        Self {
            stdout: 0,
            stderr: 0,
            combined: 0,
        }
    }
    pub(super) fn capture(
        &mut self,
        event: &mut ProcessStreamEvent,
        settings: &Settings,
    ) -> Option<ProcessStreamOutcome> {
        let (bytes, count, maximum, outcome) = match event {
            ProcessStreamEvent::Stdout(bytes) => (
                bytes,
                &mut self.stdout,
                settings.stdout,
                ProcessStreamOutcome::StdoutOverflow,
            ),
            ProcessStreamEvent::Stderr(bytes) => (
                bytes,
                &mut self.stderr,
                settings.stderr,
                ProcessStreamOutcome::StderrOverflow,
            ),
            _ => return None,
        };
        let remaining = maximum.saturating_sub(*count).min(
            settings
                .combined
                .map_or(usize::MAX, |maximum| maximum.saturating_sub(self.combined)),
        );
        let overflow = bytes.len() > remaining;
        bytes.truncate(remaining);
        *count += bytes.len();
        self.combined += bytes.len();
        overflow.then_some(outcome)
    }
}
