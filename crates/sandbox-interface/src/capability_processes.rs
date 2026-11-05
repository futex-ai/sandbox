//! Bounded collected, streaming, and stateless process execution.

use async_trait::async_trait;

use crate::{
    BackendReadOnlyExecRequest, BackendRunProcessRequest, BackendStreamProcessRequest,
    ProcessEventStream, ReadOnlyExecOutput, Result, SandboxProcessOutput,
};

/// Bounded collected, streaming, and stateless process execution.
#[unimock::unimock(api = SandboxProcessesMock)]
#[async_trait]
pub trait SandboxProcesses: Send + Sync {
    /// Runs one bounded argv-direct non-interactive process to completion.
    ///
    /// A streaming provider may report completion only after both the process
    /// end event and its final stream status have been validated.
    async fn run_process(&self, request: BackendRunProcessRequest) -> Result<SandboxProcessOutput>;
    /// Streams one bounded argv-direct non-interactive process.
    ///
    /// A successfully returned stream ends with exactly one terminal outcome.
    /// Provider completion requires both a process end and successful trailer.
    async fn stream_process(
        &self,
        request: BackendStreamProcessRequest,
    ) -> Result<ProcessEventStream>;
    /// Runs one bounded non-interactive read-only command without allocating a
    /// provider terminal, retaining command state, or changing sandbox lifecycle.
    /// Streaming providers must validate the final stream status after process
    /// end before returning output.
    async fn read_only_exec(
        &self,
        request: BackendReadOnlyExecRequest,
    ) -> Result<ReadOnlyExecOutput>;
}
