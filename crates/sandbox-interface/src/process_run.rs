//! Bounded argv-direct non-interactive process execution values.

use std::{pin::Pin, time::Duration};

use futures_core::Stream;

use crate::{ProviderRef, ResourceOwner, SandboxId};

/// Maximum bytes accepted for one captured process stream.
pub const PROCESS_RUN_MAX_STREAM_BYTES: usize = 64 * 1024 * 1024;

/// Maximum total UTF-8 bytes across the command and every argument.
pub const PROCESS_RUN_MAX_ARGV_BYTES: usize = 128 * 1024;

/// Maximum execution deadline accepted for one process run.
pub const PROCESS_RUN_MAX_DEADLINE: Duration = Duration::from_secs(300);

/// Maximum absolute deadline accepted for one streaming process run.
pub const PROCESS_STREAM_MAX_DEADLINE: Duration = Duration::from_secs(3600);

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
///
/// Backends must reject an empty command, combined command and argument bytes
/// above [`PROCESS_RUN_MAX_ARGV_BYTES`], stream limits above
/// [`PROCESS_RUN_MAX_STREAM_BYTES`], and deadlines above
/// [`PROCESS_RUN_MAX_DEADLINE`] before contacting their provider.
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

/// Request to stream one bounded non-interactive process in an owned sandbox.
///
/// The absolute deadline bounds the complete stream, while `idle_timeout`
/// bounds time without stdout or stderr data. Both timers begin when execution
/// starts. The idle timeout must be nonzero and no greater than the deadline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamProcessRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Stable sandbox handle.
    pub sandbox_id: SandboxId,
    /// Executable path or `PATH`-resolved command name.
    pub command: String,
    /// Exact argument vector.
    pub args: Vec<String>,
    /// Maximum emitted stdout bytes before overflow terminates the stream.
    pub stdout_limit: usize,
    /// Maximum emitted stderr bytes before overflow terminates the stream.
    pub stderr_limit: usize,
    /// Absolute execution budget for the complete stream.
    pub deadline: Duration,
    /// Maximum duration without stdout or stderr data.
    pub idle_timeout: Duration,
}

/// Provider request to stream one bounded non-interactive process.
///
/// Backends must validate argv and stream limits identically to
/// [`BackendRunProcessRequest`]. They must also reject a deadline above
/// [`PROCESS_STREAM_MAX_DEADLINE`], a zero idle timeout, or an idle timeout
/// above the deadline before contacting their provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendStreamProcessRequest {
    /// Source provider sandbox reference.
    pub sandbox_provider_ref: ProviderRef,
    /// Executable path or `PATH`-resolved command name.
    pub command: String,
    /// Exact argument vector.
    pub args: Vec<String>,
    /// Maximum emitted stdout bytes before overflow terminates the stream.
    pub stdout_limit: usize,
    /// Maximum emitted stderr bytes before overflow terminates the stream.
    pub stderr_limit: usize,
    /// Absolute execution budget for the complete stream.
    pub deadline: Duration,
    /// Maximum duration without stdout or stderr data.
    pub idle_timeout: Duration,
}

/// Terminal result of a streaming process run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessStreamOutcome {
    /// A process end and the provider success trailer were both observed.
    Completed,
    /// Stdout exceeded its requested byte limit.
    StdoutOverflow,
    /// Stderr exceeded its requested byte limit.
    StderrOverflow,
    /// No stdout or stderr data arrived within the idle timeout.
    IdleTimeout,
    /// The absolute execution budget expired.
    DeadlineExpired,
    /// The provider stream failed or violated its framing contract.
    TransportFailure,
}

/// Ordered event emitted by one streaming process run.
///
/// `Outcome` is emitted exactly once and is always the final stream item.
/// `Exited` is not terminal because provider completion still requires a
/// decoded success trailer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessStreamEvent {
    /// The provider started the process with a nonzero operating-system PID.
    Started {
        /// Provider process identifier.
        pid: u32,
    },
    /// Exact stdout bytes, bounded cumulatively by the request.
    Stdout(Vec<u8>),
    /// Exact stderr bytes, bounded cumulatively by the request.
    Stderr(Vec<u8>),
    /// The provider reported that the process ended.
    Exited {
        /// Provider-reported process exit code.
        exit_code: i32,
    },
    /// Final typed result. No event follows this one.
    Outcome(ProcessStreamOutcome),
}

/// Boxed asynchronous event stream returned by process streaming methods.
pub type ProcessEventStream = Pin<Box<dyn Stream<Item = ProcessStreamEvent> + Send + 'static>>;

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
