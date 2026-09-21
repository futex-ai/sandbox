//! Bounded argv-direct non-interactive process execution values.

use std::time::Duration;

use crate::{ProviderRef, ResourceOwner, SandboxId};

/// Maximum bytes accepted for one captured process stream.
pub const PROCESS_RUN_MAX_STREAM_BYTES: usize = 64 * 1024 * 1024;

/// Maximum total UTF-8 bytes across the command and every argument.
pub const PROCESS_RUN_MAX_ARGV_BYTES: usize = 128 * 1024;

/// Maximum execution deadline accepted for one process run.
pub const PROCESS_RUN_MAX_DEADLINE: Duration = Duration::from_secs(300);

/// Request to run one bounded non-interactive process in an owned sandbox.
///
/// Execution is argv-direct and shell-free: the command and arguments are
/// passed to the provider verbatim, with no shell interpretation, PTY,
/// terminal, or transcript row. Trusted services use this boundary for driver
/// and helper invocations; it is never model-reachable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunProcessRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Stable sandbox handle.
    pub sandbox_id: SandboxId,
    /// Executable path or `PATH`-resolved command name.
    pub command: String,
    /// Exact argument vector.
    pub args: Vec<String>,
    /// Maximum captured stdout bytes before the run reports overflow.
    pub stdout_limit: usize,
    /// Maximum captured stderr bytes before the run reports overflow.
    pub stderr_limit: usize,
    /// Execution deadline; expiry returns a non-exited output, never a hang.
    pub deadline: Duration,
}

/// Provider request to run one bounded non-interactive process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendRunProcessRequest {
    /// Source provider sandbox reference.
    pub sandbox_provider_ref: ProviderRef,
    /// Executable path or `PATH`-resolved command name.
    pub command: String,
    /// Exact argument vector.
    pub args: Vec<String>,
    /// Maximum captured stdout bytes before the run reports overflow.
    pub stdout_limit: usize,
    /// Maximum captured stderr bytes before the run reports overflow.
    pub stderr_limit: usize,
    /// Execution deadline; expiry returns a non-exited output, never a hang.
    pub deadline: Duration,
}

/// Bounded result of one non-interactive process run.
///
/// Overflow and deadline expiry are reported as data, never silently
/// truncated into a success: callers must treat `stdout_overflowed`,
/// `stderr_overflowed`, and a missing exit as typed failures before parsing
/// any captured bytes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SandboxProcessOutput {
    /// Captured stdout bytes up to the requested limit.
    pub stdout: Vec<u8>,
    /// Captured stderr bytes up to the requested limit.
    pub stderr: Vec<u8>,
    /// Exit code when the process terminalized inside the deadline.
    pub exit_code: Option<i32>,
    /// Whether an end event was observed inside the deadline.
    pub exited: bool,
    /// Whether stdout exceeded its capture limit.
    pub stdout_overflowed: bool,
    /// Whether stderr exceeded its capture limit.
    pub stderr_overflowed: bool,
}
