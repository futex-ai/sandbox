//! Categories carried by the shared handled error contract.

use serde::{Deserialize, Serialize};

/// Runtime resource category used in handled errors.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    /// Sandbox resource.
    Sandbox,
    /// Snapshot resource.
    Snapshot,
    /// Terminal resource.
    Terminal,
    /// Terminal action resource.
    Action,
    /// Regular file resource.
    File,
}

/// Code-owned quota category.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaKind {
    /// Active sandboxes owned by one agent.
    AgentSandboxes,
    /// Active agent-owned sandboxes in one workspace.
    WorkspaceSandboxes,
    /// Retained snapshots owned by one agent.
    AgentSnapshots,
    /// Retained agent-owned snapshots in one workspace.
    WorkspaceSnapshots,
    /// Concurrent agent-owned snapshot operations in one workspace.
    WorkspaceSnapshotOperations,
    /// Active terminals in one sandbox.
    SandboxTerminals,
    /// Active browser-consumer sandboxes in one workspace.
    WorkspaceBrowserSandboxes,
}
