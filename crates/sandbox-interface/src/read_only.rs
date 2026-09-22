//! Stateless sandbox read capability, requests, and results.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;

use crate::{ProviderRef, ResourceOwner, Result, SandboxId, SandboxState};

/// Request to verify one owned sandbox's provider-visible read state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectReadOnlySandboxRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Existing sandbox target.
    pub sandbox_id: SandboxId,
}

/// Provider-visible state for an owned sandbox read target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadOnlySandbox {
    /// Stable sandbox identifier.
    pub sandbox_id: SandboxId,
    /// Current provider-neutral lifecycle state.
    pub state: SandboxState,
}

/// Stateless, non-interactive command executed in an already-live sandbox.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadOnlyExecRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Existing live sandbox target.
    pub sandbox_id: SandboxId,
    /// Absolute working directory selected by trusted service code.
    pub cwd: String,
    /// Exact executable path; no shell interpretation occurs.
    pub executable: String,
    /// Exact argument vector passed to the executable.
    pub args: Vec<String>,
    /// Maximum combined stdout and stderr bytes returned.
    pub output_limit: usize,
    /// Maximum command duration in seconds.
    pub timeout_seconds: u64,
}

/// Bounded combined output from a stateless read-only command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadOnlyExecOutput {
    /// Combined stdout and stderr bytes in provider order.
    pub bytes: Vec<u8>,
    /// Process exit code.
    pub exit_code: i32,
}

/// Bounded regular-file read in an already-live owned sandbox.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadOnlyFileRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Existing live sandbox target.
    pub sandbox_id: SandboxId,
    /// Absolute trusted root selected by service code.
    pub root: String,
    /// Normalized path relative to `root`.
    pub path: String,
    /// Maximum file bytes returned by this call.
    pub output_limit: usize,
}

/// Exact bounded bytes and complete-size metadata from a regular file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadOnlyFileOutput {
    /// Exact bytes returned from offset zero.
    pub bytes: Vec<u8>,
    /// Complete regular-file size observed by the provider.
    pub total_size: u64,
    /// Whether bytes remain beyond the returned prefix.
    pub truncated: bool,
}

/// Provider request for a stateless, non-interactive read-only command.
///
/// Backends must reject an empty executable, combined executable and argument
/// bytes above [`PROCESS_RUN_MAX_ARGV_BYTES`](crate::PROCESS_RUN_MAX_ARGV_BYTES),
/// an output limit above
/// [`PROCESS_RUN_MAX_STREAM_BYTES`](crate::PROCESS_RUN_MAX_STREAM_BYTES), and a
/// timeout above [`PROCESS_RUN_MAX_DEADLINE`](crate::PROCESS_RUN_MAX_DEADLINE)
/// before acquiring provider access.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendReadOnlyExecRequest {
    /// Source provider sandbox reference.
    pub sandbox_provider_ref: ProviderRef,
    /// Absolute working directory selected by trusted service code.
    pub cwd: String,
    /// Exact executable path; no shell interpretation occurs.
    pub executable: String,
    /// Exact argument vector passed to the executable.
    pub args: Vec<String>,
    /// Maximum combined stdout and stderr bytes returned, capped by
    /// [`PROCESS_RUN_MAX_STREAM_BYTES`](crate::PROCESS_RUN_MAX_STREAM_BYTES).
    pub output_limit: usize,
    /// Maximum command duration, bounded by
    /// [`PROCESS_RUN_MAX_DEADLINE`](crate::PROCESS_RUN_MAX_DEADLINE).
    pub timeout: Duration,
}

/// Narrow capability for lifecycle-invisible reads from existing sandboxes.
///
/// Implementations must never create or restore a sandbox, allocate a terminal
/// or session, extend provider lifetime, or mutate durable activity state.
#[unimock::unimock(api = SandboxReadOnlyMock)]
#[async_trait]
pub trait SandboxReadOnly: Send + Sync {
    /// Verifies an owned sandbox is currently running without resuming it or
    /// altering its provider timeout.
    async fn inspect_sandbox(
        &self,
        request: InspectReadOnlySandboxRequest,
    ) -> Result<ReadOnlySandbox>;

    /// Executes one bounded command in an owned, already-running sandbox
    /// without a session, persistence, resume, or lifetime extension.
    async fn read_only_exec(&self, request: ReadOnlyExecRequest) -> Result<ReadOnlyExecOutput>;

    /// Reads one bounded regular file below a trusted root in an owned,
    /// already-running sandbox without resume or lifetime extension.
    async fn read_only_file(&self, request: ReadOnlyFileRequest) -> Result<ReadOnlyFileOutput>;
}

/// Shared lifecycle-invisible sandbox-read capability.
pub type DynSandboxReadOnly = Arc<dyn SandboxReadOnly>;
