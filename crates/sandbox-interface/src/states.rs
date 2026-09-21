//! Provider-neutral lifecycle state sets.

use serde::{Deserialize, Serialize};

/// Sandbox lifecycle state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxState {
    /// Provider allocation is in progress.
    Creating,
    /// The sandbox accepts terminal work.
    Ready,
    /// Provider state is suspended and may resume transparently.
    Paused,
    /// Destruction is in progress.
    Destroying,
    /// Provider destruction is complete.
    Destroyed,
    /// The resource cannot continue normally.
    Failed,
}

/// Snapshot lifecycle state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotState {
    /// Provider capture is in progress.
    Creating,
    /// The snapshot may create new sandboxes.
    Ready,
    /// Provider identity cannot yet be proven safely.
    ReconciliationRequired,
    /// Deletion is in progress.
    Deleting,
    /// Provider deletion is complete.
    Deleted,
    /// The snapshot operation failed permanently.
    Failed,
}

/// Terminal lifecycle state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalState {
    /// Provider process creation is in progress.
    Creating,
    /// The terminal accepts commands and input.
    Ready,
    /// Close is in progress.
    Closing,
    /// Explicit close completed.
    Closed,
    /// The shell exited independently.
    Exited,
    /// The terminal cannot continue normally.
    Failed,
}

/// Terminal action kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalActionKind {
    /// A shell command with a completion marker.
    Execute,
    /// Exact interactive input.
    Write,
}

/// Terminal action lifecycle state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalActionState {
    /// The durable action exists but is not dispatched.
    Pending,
    /// Provider dispatch is running.
    Running,
    /// The bounded caller wait expired while the action continued.
    Waiting,
    /// Completion was observed.
    Completed,
    /// The action failed without ambiguous delivery.
    Failed,
    /// Provider delivery may have occurred and must not be replayed.
    DeliveryUnknown,
}

/// Multi-worker cleanup lifecycle state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupState {
    /// Cleanup is not required.
    NotScheduled,
    /// Cleanup is ready to claim.
    Pending,
    /// One worker owns a bounded claim lease.
    Claimed,
    /// A transient failure should be retried later.
    Retryable,
    /// Cleanup completed.
    Complete,
}
