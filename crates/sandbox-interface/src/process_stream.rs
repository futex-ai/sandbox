//! Bounded incremental non-interactive process execution values.

use std::{pin::Pin, time::Duration};

use futures_core::Stream;

use crate::{BackendRunProcessRequest, ProviderRef, ResourceOwner, SandboxId};

/// Maximum absolute deadline accepted for one streaming process run.
pub const PROCESS_STREAM_MAX_DEADLINE: Duration = Duration::from_secs(3600);

/// Request to stream one bounded non-interactive process in an owned sandbox.
///
/// The absolute deadline includes backend sandbox connection and the complete
/// stream. The idle timer begins before opening the process transport and
/// bounds time without stdout or stderr data. It must be nonzero and no greater
/// than the requested deadline.
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
    /// Absolute execution budget, including backend sandbox connection.
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
    /// Absolute execution budget, including backend sandbox connection.
    pub deadline: Duration,
    /// Maximum duration without stdout or stderr data.
    pub idle_timeout: Duration,
}

/// Terminal result of a streaming process run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessStreamOutcome {
    /// A process end, provider success trailer, and transport EOF were observed.
    ///
    /// This confirms protocol completion, not a successful command exit. The
    /// preceding [`ProcessStreamEvent::Exited`] event carries that distinction.
    Completed,
    /// Stdout exceeded its requested byte limit.
    StdoutOverflow,
    /// Stderr exceeded its requested byte limit.
    StderrOverflow,
    /// No stdout or stderr data arrived within the idle timeout.
    IdleTimeout,
    /// The consumer could not keep up with bounded provider output staging.
    ConsumerBackpressure,
    /// The absolute execution budget expired.
    DeadlineExpired,
    /// The provider stream failed or violated its framing contract.
    TransportFailure,
}

/// Ordered event emitted by one streaming process run.
///
/// `Outcome` is emitted exactly once and is always the final stream item.
/// `Exited` is not terminal because provider completion still requires a
/// decoded success trailer followed by transport EOF.
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
    /// The provider reported that the process ended, normally or by signal.
    Exited {
        /// Provider-reported process exit code.
        exit_code: i32,
        /// Whether the process exited normally rather than by signal.
        exited: bool,
    },
    /// Final typed result. No event follows this one.
    Outcome(ProcessStreamOutcome),
}

/// Boxed asynchronous event stream returned by process streaming methods.
pub type ProcessEventStream = Pin<Box<dyn Stream<Item = ProcessStreamEvent> + Send + 'static>>;
