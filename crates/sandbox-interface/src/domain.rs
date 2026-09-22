//! Provider-neutral durable records and results.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    ActionId, CleanupState, ProviderRef, SandboxConsumer, SandboxId, SandboxState, SnapshotId,
    SnapshotState, TerminalActionKind, TerminalActionState, TerminalId, TerminalState,
};

/// Workspace-scoped owner of one runtime resource.
///
/// An absent agent identifies a platform-owned resource shared internally
/// across the workspace. Model-visible resources always carry an agent.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceOwner {
    /// Owning workspace UUID.
    pub workspace_id: Uuid,
    /// Owning execution-agent UUID, absent for workspace-platform resources.
    pub agent_id: Option<Uuid>,
}

impl ResourceOwner {
    /// Creates an execution-agent owner.
    #[must_use]
    pub const fn agent(workspace_id: Uuid, agent_id: Uuid) -> Self {
        Self {
            workspace_id,
            agent_id: Some(agent_id),
        }
    }

    /// Creates a workspace-platform owner with no agent principal.
    #[must_use]
    pub const fn platform(workspace_id: Uuid) -> Self {
        Self {
            workspace_id,
            agent_id: None,
        }
    }

    /// Returns whether the resource belongs to the workspace platform.
    #[must_use]
    pub const fn is_platform(self) -> bool {
        self.agent_id.is_none()
    }
}

/// Provider-neutral sandbox record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Sandbox {
    /// Stable consumer handle.
    pub sandbox_id: SandboxId,
    /// Runtime owner.
    pub owner: ResourceOwner,
    /// Substrate consumer class recorded at creation.
    pub consumer: SandboxConsumer,
    /// Stable configured backend ID used for routing.
    pub backend_id: String,
    /// Deployment-owned logical profile.
    pub profile: String,
    /// Snapshot used to create this sandbox, when applicable.
    pub source_snapshot_id: Option<SnapshotId>,
    /// Current lifecycle state.
    pub state: SandboxState,
    /// Cleanup claim state.
    pub cleanup_state: CleanupState,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last state-change timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Provider-neutral reusable snapshot record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SandboxSnapshot {
    /// Stable consumer handle.
    pub snapshot_id: SnapshotId,
    /// Runtime owner inherited from the source.
    pub owner: ResourceOwner,
    /// Stable source sandbox handle.
    pub source_sandbox_id: SandboxId,
    /// Stored backend route.
    pub backend_id: String,
    /// Stored logical profile.
    pub profile: String,
    /// Optional consumer-only model label.
    pub label: Option<String>,
    /// Current lifecycle state.
    pub state: SnapshotState,
    /// Cleanup claim state.
    pub cleanup_state: CleanupState,
    /// Observed retained image bytes, when measured by platform realization.
    pub size_bytes: Option<u64>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last state-change timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Provider-neutral persistent terminal record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Terminal {
    /// Stable consumer handle.
    pub terminal_id: TerminalId,
    /// Owning sandbox handle.
    pub sandbox_id: SandboxId,
    /// Runtime owner repeated for constrained lookup.
    pub owner: ResourceOwner,
    /// Current lifecycle state.
    pub state: TerminalState,
    /// Cleanup claim state.
    pub cleanup_state: CleanupState,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last state-change timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Owned terminal with redaction-safe active-action metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalStatus {
    /// Persistent terminal record.
    pub terminal: Terminal,
    /// Active action, when the terminal is occupied.
    pub active_action_id: Option<ActionId>,
    /// Active action state, when the terminal is occupied.
    pub active_action_state: Option<TerminalActionState>,
}

/// Durable terminal action record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalAction {
    /// Stable consumer action handle.
    pub action_id: ActionId,
    /// Owning terminal handle.
    pub terminal_id: TerminalId,
    /// Runtime owner repeated for constrained lookup.
    pub owner: ResourceOwner,
    /// Action kind.
    pub kind: TerminalActionKind,
    /// Current action state.
    pub state: TerminalActionState,
    /// Observed shell exit code for completed execute actions.
    pub exit_code: Option<i32>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last state-change timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Bounded UTF-8 terminal transcript window.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct TranscriptWindow {
    /// Owning terminal handle.
    pub terminal_id: TerminalId,
    /// Current terminal state.
    pub terminal_state: TerminalState,
    /// Actual retained start offset returned.
    pub offset: u64,
    /// Offset for the next read.
    pub next_offset: u64,
    /// Monotonic transcript revision.
    pub revision: u64,
    /// Normalized UTF-8 content.
    pub text: String,
    /// UTF-8 byte count in `text`.
    pub returned_bytes: usize,
    /// Whether requested or older data was evicted.
    pub truncated: bool,
    /// Related durable action, when one was requested.
    pub action_id: Option<ActionId>,
    /// Current related action state.
    pub action_status: Option<TerminalActionState>,
}

/// Execute or write result with a bounded transcript view.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalActionResult {
    /// Stable action handle.
    pub action_id: ActionId,
    /// Current action state.
    pub action_status: TerminalActionState,
    /// Observed shell exit code for a completed execute.
    pub exit_code: Option<i32>,
    /// Latest transcript window.
    pub transcript: TranscriptWindow,
}

/// Bounded regular-file content returned by the sandbox service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileContent {
    /// Absolute offset used for this read.
    pub offset: u64,
    /// Exact returned bytes.
    pub bytes: Vec<u8>,
    /// Complete regular-file size.
    pub total_size: u64,
    /// Whether unread bytes remain after this window.
    pub truncated: bool,
}

/// Result of realizing one workspace-platform image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealizedImage {
    /// Durable platform-owned snapshot.
    pub image: SandboxSnapshot,
    /// Backend-observed retained image size in bytes.
    pub size_bytes: u64,
}

/// Provider runtime retained after a failed workspace-platform image build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedSandboxRef {
    /// Stable consumer sandbox handle that should own the retained provider runtime.
    pub sandbox_id: SandboxId,
    /// Provider identity of the retained failed build runtime.
    pub provider_ref: ProviderRef,
}
