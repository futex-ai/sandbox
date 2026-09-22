//! Authorized provider-neutral lifecycle service.

use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    AdoptSandboxRequest, CloseTerminalRequest, CreateSandboxRequest, CreateSnapshotRequest,
    CreateTerminalRequest, DeleteSnapshotRequest, DestroySandboxRequest, EnsureScreenStackRequest,
    ExecuteTerminalRequest, FileContent, ListSandboxesRequest, ListSnapshotsRequest,
    ListTerminalsRequest, PortIngress, PortIngressRequest, ProcessEventStream, ReadFileRequest,
    ReadTerminalActionRequest, ReadTerminalRequest, RealizeImageRequest, RealizedImage,
    ResizeScreenStackRequest, Result, RunProcessRequest, Sandbox, SandboxLifecycleRequest,
    SandboxProcessOutput, SandboxSnapshot, SandboxTerminalReplacementRequest, ScreenStackOutcome,
    ScreenViewportSize, StreamProcessRequest, Terminal, TerminalActionResult, TerminalStatus,
    TerminalStatusRequest, TranscriptWindow, WriteFileRequest, WriteTerminalRequest,
};

/// Lifecycle boundary consumed by trusted environment and fn-run services.
#[unimock::unimock(api = SandboxServiceMock)]
#[async_trait]
pub trait SandboxService: Send + Sync {
    /// Realizes one workspace-platform image through its configured backend.
    async fn realize_image(&self, request: RealizeImageRequest) -> Result<RealizedImage>;
    /// Creates a sandbox from one logical profile or owned snapshot.
    async fn create_sandbox(&self, request: CreateSandboxRequest) -> Result<Sandbox>;
    /// Transfers one retained platform sandbox to an execution agent.
    async fn adopt_sandbox(&self, request: AdoptSandboxRequest) -> Result<Sandbox>;
    /// Lists sandboxes in the requested ownership class.
    async fn list_sandboxes(&self, request: ListSandboxesRequest) -> Result<Vec<Sandbox>>;
    /// Idempotently destroys one owned sandbox and its terminals.
    async fn destroy_sandbox(&self, request: DestroySandboxRequest) -> Result<Sandbox>;
    /// Resolves one owned ready sandbox port into a call-local upstream.
    async fn port_ingress(&self, request: PortIngressRequest) -> Result<PortIngress>;
    /// Ensures the screen stack when the owned sandbox profile supports it.
    async fn ensure_screen_stack(
        &self,
        request: EnsureScreenStackRequest,
    ) -> Result<ScreenStackOutcome>;
    /// Idempotently applies one exact viewport to a supported screen stack.
    async fn resize_screen_stack(
        &self,
        request: ResizeScreenStackRequest,
    ) -> Result<ScreenViewportSize>;
    /// Captures one owned sandbox as a reusable snapshot.
    async fn create_snapshot(&self, request: CreateSnapshotRequest) -> Result<SandboxSnapshot>;
    /// Lists snapshots in the requested ownership class.
    async fn list_snapshots(&self, request: ListSnapshotsRequest) -> Result<Vec<SandboxSnapshot>>;
    /// Idempotently deletes one owned snapshot.
    async fn delete_snapshot(&self, request: DeleteSnapshotRequest) -> Result<SandboxSnapshot>;
    /// Runs one bounded argv-direct non-interactive process in an owned sandbox.
    async fn run_process(&self, request: RunProcessRequest) -> Result<SandboxProcessOutput>;
    /// Streams one bounded argv-direct non-interactive process in an owned sandbox.
    async fn stream_process(&self, request: StreamProcessRequest) -> Result<ProcessEventStream>;
    /// Reads a bounded regular file below a trusted root in an owned sandbox.
    async fn read_file(&self, request: ReadFileRequest) -> Result<FileContent>;
    /// Replaces a bounded regular file below a trusted root in an owned sandbox.
    async fn write_file(&self, request: WriteFileRequest) -> Result<()>;
    /// Creates a persistent terminal in one owned sandbox.
    async fn create_terminal(&self, request: CreateTerminalRequest) -> Result<Terminal>;
    /// Lists active terminals and redaction-safe action occupancy in one owned sandbox.
    async fn list_terminals(&self, request: ListTerminalsRequest) -> Result<Vec<TerminalStatus>>;
    /// Reconciles one exact owned terminal even when it is no longer active.
    async fn terminal_status(&self, request: TerminalStatusRequest) -> Result<TerminalStatus>;
    /// Claims or renews an authorized sandbox lifecycle fence.
    async fn claim_lifecycle(&self, request: SandboxLifecycleRequest) -> Result<()>;
    /// Claims a lifecycle fence that permits continuations on other terminals.
    async fn claim_terminal_replacement(
        &self,
        request: SandboxTerminalReplacementRequest,
    ) -> Result<()>;
    /// Releases an authorized sandbox lifecycle fence owned by this operation.
    async fn release_lifecycle(&self, request: SandboxLifecycleRequest) -> Result<()>;
    /// Executes a command and returns bounded transcript output.
    async fn execute_terminal(
        &self,
        request: ExecuteTerminalRequest,
    ) -> Result<TerminalActionResult>;
    /// Reads or briefly waits for a bounded transcript window.
    async fn read_terminal(&self, request: ReadTerminalRequest) -> Result<TranscriptWindow>;
    /// Reads and reconciles one owned durable execute action.
    async fn read_terminal_action(
        &self,
        request: ReadTerminalActionRequest,
    ) -> Result<TerminalActionResult>;
    /// Sends exact interactive input and never appends a newline.
    async fn write_terminal(&self, request: WriteTerminalRequest) -> Result<TerminalActionResult>;
    /// Idempotently closes one terminal.
    async fn close_terminal(&self, request: CloseTerminalRequest) -> Result<Terminal>;
}

/// Shared dynamic sandbox service alias.
pub type DynSandboxService = Arc<dyn SandboxService>;
