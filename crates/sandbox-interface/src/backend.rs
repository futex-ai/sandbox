//! Required swappable sandbox-provider contract.

use std::sync::Arc;

use async_trait::async_trait;

use crate::backend_terminal::{
    BackendInputRequest, BackendOutputRequest, BackendTerminal, BackendTerminalCreateRequest,
    BackendTerminalOutput,
};
use crate::{
    BackendEnsureScreenStackRequest, BackendFileContent, BackendPortIngressRequest,
    BackendPrepareImageRequest, BackendPreparedImage, BackendReadFileRequest,
    BackendReadOnlyExecRequest, BackendResizeScreenStackRequest, BackendRunProcessRequest,
    BackendWriteFileRequest, OperationId, PortIngress, ProviderRef, ReadOnlyExecOutput,
    ResourceOwner, Result, SandboxConsumer, SandboxId, SandboxLifetime, SandboxNetworkPolicy,
    SandboxProcessOutput, SandboxState, ScreenStackOutcome, ScreenViewportSize, SnapshotId,
    SnapshotState,
};

/// Provider request to create a sandbox.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendCreateSandboxRequest {
    /// Stable consumer sandbox correlation handle.
    pub sandbox_id: SandboxId,
    /// Durable operation correlation handle.
    pub operation_id: OperationId,
    /// Opaque ownership metadata.
    pub owner: ResourceOwner,
    /// Substrate consumer class preserved in provider metadata.
    pub consumer: SandboxConsumer,
    /// Provider-neutral lifetime policy revalidated before provider dispatch.
    pub lifetime: SandboxLifetime,
    /// Deployment identity used only as opaque metadata.
    pub deployment_id: String,
    /// Logical profile resolved by the adapter to provider configuration.
    pub profile: String,
    /// Typed per-session network policy revalidated before provider dispatch.
    pub network: SandboxNetworkPolicy,
    /// Provider snapshot ref for a restore, when applicable.
    pub snapshot_provider_ref: Option<ProviderRef>,
}

/// Provider sandbox state and opaque identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendSandbox {
    /// Opaque provider sandbox reference.
    pub provider_ref: ProviderRef,
    /// Mapped lifecycle state.
    pub state: SandboxState,
}

/// Provider sandbox discovered through deployment-scoped reconciliation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendManagedSandbox {
    /// Opaque provider sandbox reference.
    pub provider_ref: ProviderRef,
    /// Mapped provider lifecycle state.
    pub state: SandboxState,
    /// consumer sandbox handle recovered from opaque provider metadata.
    pub sandbox_id: Option<SandboxId>,
    /// Durable operation handle recovered from opaque provider metadata.
    pub operation_id: Option<OperationId>,
    /// Consumer class recovered from metadata, absent on legacy resources.
    pub consumer: Option<SandboxConsumer>,
    /// Lifetime recovered from metadata, absent when missing or malformed.
    pub lifetime: Option<SandboxLifetime>,
}

/// Provider snapshot state and opaque identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendSnapshot {
    /// Opaque provider snapshot reference.
    pub provider_ref: ProviderRef,
    /// Mapped lifecycle state.
    pub state: SnapshotState,
}

/// Source-and-correlation-filtered provider snapshot inventory.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BackendSnapshotInventory {
    /// Matching opaque snapshot refs in deterministic provider order.
    pub snapshots: Vec<ProviderRef>,
}

/// Provider request to inspect one snapshot within its durable source context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendInspectSnapshotRequest {
    /// Opaque provider snapshot reference.
    pub provider_ref: ProviderRef,
    /// Opaque provider source-sandbox reference.
    pub source_provider_ref: ProviderRef,
    /// Opaque provider correlation name recorded before snapshot dispatch.
    pub correlation_name: String,
}

/// Provider snapshot creation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendCreateSnapshotRequest {
    /// Stable consumer snapshot handle.
    pub snapshot_id: SnapshotId,
    /// Durable operation handle.
    pub operation_id: OperationId,
    /// Source provider sandbox reference.
    pub source_provider_ref: ProviderRef,
    /// Opaque provider correlation name derived from the operation ID.
    pub correlation_name: String,
    /// Inventory captured before dispatch.
    pub before: BackendSnapshotInventory,
}

/// Immediate provider result for snapshot creation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendSnapshotCreateOutcome {
    /// Provider returned one identifiable snapshot.
    Created(BackendSnapshot),
    /// Provider accepted work that remains in progress.
    InProgress,
    /// Transport outcome is ambiguous and requires inventory reconciliation.
    DeliveryAmbiguous,
}

/// At-most-once reconciliation result after ambiguous snapshot delivery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendSnapshotRecovery {
    /// Exactly one deterministic new provider candidate was proven.
    Recovered(BackendSnapshot),
    /// The matching provider operation is still in progress.
    InProgress,
    /// Multiple candidates or another terminal conflict prevent safe adoption.
    ReconciliationRequired,
}

/// Mandatory backend behavior for every configured sandbox implementation.
#[unimock::unimock(api = SandboxBackendMock)]
#[async_trait]
pub trait SandboxBackend: Send + Sync {
    /// Prepares one durably recorded platform image source for snapshot dispatch.
    ///
    /// The caller creates or recovers the source in separate durable phases,
    /// invokes this preparation phase at most once, persists its result, then
    /// uses the snapshot inventory, create, and recovery methods below. This
    /// method never allocates, snapshots, or destroys a provider resource.
    /// Provider-owned safety phases must not load user-controlled shell startup
    /// files before they run.
    async fn prepare_image(
        &self,
        request: BackendPrepareImageRequest,
    ) -> Result<BackendPreparedImage>;
    /// Lists provider sandboxes correlated to one consumer deployment.
    async fn list_managed_sandboxes(
        &self,
        deployment_id: String,
    ) -> Result<Vec<BackendManagedSandbox>>;
    /// Dispatches a new sandbox create after the caller records create intent.
    ///
    /// After local validation, the backend must send its one create mutation
    /// without a fallible provider inventory preflight. Ambiguous delivery may
    /// use recovery reads but must not dispatch a second mutation.
    ///
    /// Once invocation starts, retries must call `recover_sandbox_create`
    /// instead of this method until the outcome is reconciled.
    async fn create_sandbox(&self, request: BackendCreateSandboxRequest) -> Result<BackendSandbox>;
    /// Reconciles a correlated sandbox create without dispatching a new create.
    async fn recover_sandbox_create(
        &self,
        request: BackendCreateSandboxRequest,
    ) -> Result<Option<BackendSandbox>>;
    /// Inspects one provider sandbox.
    async fn inspect_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox>;
    /// Resumes or reconnects one provider sandbox.
    ///
    /// A one-shot sandbox may be verified only while already running; this
    /// operation must not resume it or extend its original maximum lifetime.
    async fn resume_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox>;
    /// Resolves one exact port into a call-local authenticated upstream.
    async fn port_ingress(&self, request: BackendPortIngressRequest) -> Result<PortIngress>;
    /// Idempotently ensures a screen-capable sandbox's bridge stack.
    async fn ensure_screen_stack(
        &self,
        request: BackendEnsureScreenStackRequest,
    ) -> Result<ScreenStackOutcome>;
    /// Idempotently applies one exact viewport to a screen-capable sandbox.
    async fn resize_screen_stack(
        &self,
        request: BackendResizeScreenStackRequest,
    ) -> Result<ScreenViewportSize>;
    /// Pauses one provider sandbox when supported; one-shot sandboxes reject it.
    async fn pause_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox>;
    /// Idempotently destroys one provider sandbox.
    async fn destroy_sandbox(&self, provider_ref: ProviderRef) -> Result<()>;
    /// Lists snapshots filtered to one source and opaque correlation name.
    async fn snapshot_inventory(
        &self,
        source_provider_ref: ProviderRef,
        correlation_name: String,
    ) -> Result<BackendSnapshotInventory>;
    /// Dispatches snapshot creation once after the caller records dispatch intent.
    ///
    /// Once invocation starts, retries must call `recover_snapshot_create`
    /// with this exact request instead of redispatching it.
    async fn create_snapshot(
        &self,
        request: BackendCreateSnapshotRequest,
    ) -> Result<BackendSnapshotCreateOutcome>;
    /// Reconciles ambiguous snapshot creation without replaying it.
    ///
    /// A provider that pauses the source during capture must keep it paused for
    /// `InProgress` or `ReconciliationRequired` and may reconnect only after
    /// proving one completed snapshot.
    async fn recover_snapshot_create(
        &self,
        request: BackendCreateSnapshotRequest,
    ) -> Result<BackendSnapshotRecovery>;
    /// Inspects one provider snapshot using its durable source context.
    async fn inspect_snapshot(
        &self,
        request: BackendInspectSnapshotRequest,
    ) -> Result<BackendSnapshot>;
    /// Idempotently deletes one provider snapshot.
    async fn delete_snapshot(&self, provider_ref: ProviderRef) -> Result<()>;
    /// Removes inherited consumer terminal helpers from a restored sandbox.
    async fn clean_restored_terminals(&self, sandbox_provider_ref: ProviderRef) -> Result<()>;
    /// Runs one bounded argv-direct non-interactive process to completion.
    ///
    /// A streaming provider may report completion only after both the process
    /// end event and its final stream status have been validated.
    async fn run_process(&self, request: BackendRunProcessRequest) -> Result<SandboxProcessOutput>;
    /// Reads a bounded regular file without exposing provider credentials.
    async fn read_file(&self, request: BackendReadFileRequest) -> Result<BackendFileContent>;
    /// Runs one bounded non-interactive read-only command without allocating a
    /// provider terminal, retaining command state, or changing sandbox lifecycle.
    /// Streaming providers must validate the final stream status after process
    /// end before returning output.
    async fn read_only_exec(
        &self,
        request: BackendReadOnlyExecRequest,
    ) -> Result<ReadOnlyExecOutput>;
    /// Replaces a bounded regular file without exposing provider credentials.
    async fn write_file(&self, request: BackendWriteFileRequest) -> Result<()>;
    /// Creates one persistent provider PTY.
    ///
    /// Durable transcript capture must be active before a user login profile
    /// can run so startup output and exits cannot bypass terminal bookkeeping.
    /// The provider identity must also be durably recoverable before that shell
    /// can exit.
    /// The transcript limit must be validated before provider access and cannot
    /// exceed [`crate::FILE_TRANSFER_MAX_BYTES`].
    async fn create_terminal(
        &self,
        request: BackendTerminalCreateRequest,
    ) -> Result<BackendTerminal>;
    /// Recovers a correlated terminal create without allocating a new PTY,
    /// including the original provider reference in `Exited` state when only
    /// trusted identity state remains.
    ///
    /// The transcript limit has the same pre-provider-access bound as create.
    async fn recover_terminal_create(
        &self,
        request: BackendTerminalCreateRequest,
    ) -> Result<Option<BackendTerminal>>;
    /// Inspects one provider PTY, including a durably recorded exited PTY.
    async fn inspect_terminal(
        &self,
        sandbox_provider_ref: ProviderRef,
        terminal_provider_ref: ProviderRef,
    ) -> Result<BackendTerminal>;
    /// Ingests bytes from the durable provider-side terminal log.
    ///
    /// Provider-side identity reads must carry an earlier absolute completion
    /// deadline that reserves termination and return time inside this
    /// operation's bounded wait.
    async fn read_terminal(&self, request: BackendOutputRequest) -> Result<BackendTerminalOutput>;
    /// Sends exact input once; success requires a decoded provider
    /// acknowledgment, and callers fail closed after ambiguous delivery.
    async fn write_terminal(&self, request: BackendInputRequest) -> Result<()>;
    /// Idempotently closes one provider PTY while retaining durable identity
    /// for final transcript reads.
    async fn close_terminal(
        &self,
        sandbox_provider_ref: ProviderRef,
        terminal_provider_ref: ProviderRef,
    ) -> Result<()>;
}

/// Shared dynamic sandbox backend alias.
pub type DynSandboxBackend = Arc<dyn SandboxBackend>;
