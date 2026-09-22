//! Provider-neutral service requests.

use crate::{
    ActionId, OperationId, ResourceOwner, SandboxConsumer, SandboxId, SandboxNetworkPolicy,
    SnapshotId, TerminalId,
};

/// Parent source for one workspace-platform image realization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageSource {
    /// Deployment-owned logical profile.
    Profile(String),
    /// Existing platform-owned image snapshot.
    Image(SnapshotId),
}

/// Request to realize one credential-free workspace-platform image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealizeImageRequest {
    /// Workspace-platform owner.
    pub owner: ResourceOwner,
    /// Durable operation ID.
    pub operation_id: OperationId,
    /// Parent image or profile.
    pub source: ImageSource,
    /// Credential-free files staged through bounded transfer before setup.
    pub input_files: Vec<RealizeImageFileInput>,
    /// Credential-free setup script.
    pub setup_script: String,
    /// Ordered verification commands.
    pub verify_commands: Vec<String>,
}

/// One credential-free regular file staged before image setup runs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealizeImageFileInput {
    /// Absolute trusted transfer root.
    pub root: String,
    /// Root-relative file path.
    pub path: String,
    /// Exact replacement bytes.
    pub bytes: Vec<u8>,
}

/// Request to allocate a sandbox.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateSandboxRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Durable operation ID.
    pub operation_id: OperationId,
    /// Substrate consumer class recorded on the sandbox row.
    pub consumer: SandboxConsumer,
    /// Typed per-session network policy validated before provider dispatch.
    pub network: SandboxNetworkPolicy,
    /// Optional deployment-owned logical profile.
    pub profile: Option<String>,
    /// Optional owned snapshot source.
    pub snapshot_id: Option<SnapshotId>,
}

/// Request to transfer one retained platform sandbox to an execution agent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdoptSandboxRequest {
    /// Target execution-agent owner.
    pub owner: ResourceOwner,
    /// Stable retained platform sandbox handle.
    pub sandbox_id: SandboxId,
}

/// Request to list owned sandboxes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListSandboxesRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Substrate consumer class selected by the caller.
    pub consumer: SandboxConsumer,
    /// Whether destroyed sandbox records are included.
    pub include_destroyed: bool,
}

/// Request to destroy a sandbox.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DestroySandboxRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Durable operation ID.
    pub operation_id: OperationId,
    /// Stable sandbox handle.
    pub sandbox_id: SandboxId,
}

/// Request to capture a reusable snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateSnapshotRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Durable operation ID.
    pub operation_id: OperationId,
    /// Stable source sandbox handle.
    pub sandbox_id: SandboxId,
    /// Optional consumer-only label.
    pub label: Option<String>,
}

/// Request to list owned snapshots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListSnapshotsRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Whether deleted snapshot records are included.
    pub include_deleted: bool,
}

/// Request to delete a retained snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeleteSnapshotRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Durable operation ID.
    pub operation_id: OperationId,
    /// Stable snapshot handle.
    pub snapshot_id: SnapshotId,
}

/// Request to create a persistent terminal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateTerminalRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Durable operation ID.
    pub operation_id: OperationId,
    /// Current lifecycle-fence authority when it differs from the create identity.
    pub lifecycle_operation_id: Option<OperationId>,
    /// Stable sandbox handle.
    pub sandbox_id: SandboxId,
    /// Optional initial shell working directory.
    pub cwd: Option<String>,
}

/// Request to list active terminals in one owned sandbox.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListTerminalsRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Stable sandbox handle.
    pub sandbox_id: SandboxId,
}

/// Request to inspect and reconcile one terminal in an exact owned sandbox.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalStatusRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Expected owning sandbox for graph authorization.
    pub sandbox_id: SandboxId,
    /// Stable terminal handle.
    pub terminal_id: TerminalId,
}

/// Authorized sandbox lifecycle fence used by higher-level recovery orchestration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SandboxLifecycleRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Durable lifecycle operation identity.
    pub operation_id: OperationId,
    /// Stable sandbox handle.
    pub sandbox_id: SandboxId,
}

/// Scoped lifecycle fence for replacing one terminal while preserving other continuations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SandboxTerminalReplacementRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Current worker's lifecycle-fence authority.
    pub operation_id: OperationId,
    /// Durable candidate-creation identity shared across worker takeovers.
    pub candidate_operation_id: OperationId,
    /// Stable sandbox handle.
    pub sandbox_id: SandboxId,
    /// Terminal whose actions and lifecycle must remain fenced.
    pub blocked_terminal_id: TerminalId,
}

/// Request to execute one command in an existing terminal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecuteTerminalRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Durable operation ID.
    pub operation_id: OperationId,
    /// Stable terminal handle.
    pub terminal_id: TerminalId,
    /// Shell command text.
    pub command: String,
    /// Bounded same-wake wait in seconds.
    pub timeout_seconds: Option<u64>,
}

/// Request to read a transcript window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadTerminalRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Stable terminal handle.
    pub terminal_id: TerminalId,
    /// Requested absolute transcript offset.
    pub offset: Option<u64>,
    /// Requested bounded UTF-8 byte count.
    pub length: Option<usize>,
    /// Optional long-poll wait in seconds.
    pub wait_timeout_seconds: Option<u64>,
}

/// Request to read and reconcile one durable execute action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadTerminalActionRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Expected owning sandbox for graph authorization.
    pub sandbox_id: SandboxId,
    /// Stable execute action handle.
    pub action_id: ActionId,
    /// Requested absolute action transcript offset.
    pub offset: Option<u64>,
    /// Requested bounded UTF-8 byte count.
    pub length: Option<usize>,
    /// Optional long-poll wait in seconds.
    pub wait_timeout_seconds: Option<u64>,
}

/// Request to send exact interactive terminal input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriteTerminalRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Expected owning sandbox for graph authorization.
    pub sandbox_id: SandboxId,
    /// Durable operation ID.
    pub operation_id: OperationId,
    /// Waiting execute action that must still own the terminal.
    pub action_id: ActionId,
    /// Exact input; no newline is appended.
    pub input: String,
    /// Optional output wait in seconds.
    pub wait_timeout_seconds: Option<u64>,
}

/// Request to close a terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseTerminalRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Durable operation ID.
    pub operation_id: OperationId,
    /// Stable terminal handle.
    pub terminal_id: TerminalId,
}

/// Request to read a bounded regular file from an owned sandbox.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadFileRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Stable sandbox handle.
    pub sandbox_id: SandboxId,
    /// Absolute trusted transfer root.
    pub root: String,
    /// Root-relative file path.
    pub path: String,
    /// Absolute byte offset.
    pub offset: Option<u64>,
    /// Requested byte count.
    pub length: Option<usize>,
}

/// Request to replace a bounded regular file in an owned sandbox.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriteFileRequest {
    /// Existing lifecycle authority for trusted staging; otherwise claim an exclusive write lease.
    pub lifecycle_operation_id: Option<OperationId>,
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Stable sandbox handle.
    pub sandbox_id: SandboxId,
    /// Absolute trusted transfer root.
    pub root: String,
    /// Root-relative file path.
    pub path: String,
    /// Exact replacement bytes.
    pub bytes: Vec<u8>,
}
