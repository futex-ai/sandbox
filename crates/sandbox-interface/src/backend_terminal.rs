//! Provider-neutral terminal backend values.

use std::time::Duration;

use crate::{OperationId, ProviderRef, TerminalId, TerminalState};

/// Maximum provider-side long poll accepted for one terminal output read.
pub const TERMINAL_OUTPUT_MAX_WAIT: Duration = Duration::from_secs(30);

/// Provider terminal state and durable-log identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendTerminal {
    /// Opaque provider process reference.
    pub provider_ref: ProviderRef,
    /// Private provider-side transcript path.
    pub provider_log_path: String,
    /// Mapped lifecycle state.
    pub state: TerminalState,
}

/// Provider request to create a durable PTY.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendTerminalCreateRequest {
    /// Stable consumer terminal correlation handle.
    pub terminal_id: TerminalId,
    /// Durable operation correlation handle.
    pub operation_id: OperationId,
    /// Source provider sandbox reference.
    pub sandbox_provider_ref: ProviderRef,
    /// Optional shell working directory.
    pub cwd: Option<String>,
    /// Provider-side transcript byte cap.
    pub provider_log_limit: usize,
}

/// Provider request to ingest durable terminal output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendOutputRequest {
    /// Source provider sandbox reference.
    pub sandbox_provider_ref: ProviderRef,
    /// Provider process reference.
    pub terminal_provider_ref: ProviderRef,
    /// Private provider-side transcript path.
    pub provider_log_path: String,
    /// Durable provider-log cursor.
    pub offset: u64,
    /// Maximum bytes to ingest in this call.
    pub max_bytes: usize,
    /// Provider-side transcript byte cap used when the terminal was created.
    pub provider_log_limit: usize,
    /// Optional long-poll duration, bounded by [`TERMINAL_OUTPUT_MAX_WAIT`].
    pub wait: Duration,
}

/// Bytes and state ingested from one provider terminal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendTerminalOutput {
    /// Raw PTY bytes to normalize at the service boundary.
    pub bytes: Vec<u8>,
    /// Next durable provider-log cursor.
    pub next_offset: u64,
    /// Provider-log size observed by the same read.
    pub total_size: u64,
    /// Current provider terminal state.
    pub state: TerminalState,
    /// Exit code when known.
    pub exit_code: Option<i32>,
    /// Whether the provider-side log reached its hard cap.
    pub overflowed: bool,
}

/// Exact provider terminal-input request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendInputRequest {
    /// Source provider sandbox reference.
    pub sandbox_provider_ref: ProviderRef,
    /// Provider process reference.
    pub terminal_provider_ref: ProviderRef,
    /// Exact input bytes.
    pub input: Vec<u8>,
}
