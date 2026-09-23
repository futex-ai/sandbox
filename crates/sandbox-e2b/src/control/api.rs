//! Injectable typed control API used by the E2B backend.

use std::sync::Arc;

use async_trait::async_trait;

use crate::error::Result;

use super::{
    ControlCreateSandbox, ControlSandbox, ControlSandboxAccess, ControlSandboxReadAccess,
    ControlSnapshot, SandboxMetadata,
};

/// Typed E2B sandbox and snapshot control-plane operations.
#[unimock::unimock(api = E2bControlApiMock)]
#[async_trait]
pub trait E2bControlApi: Send + Sync {
    /// Lists DNS-route-safe running sandboxes matching exact consumer metadata.
    async fn list_sandboxes(&self, metadata: SandboxMetadata) -> Result<Vec<ControlSandbox>>;
    /// Creates one sandbox and returns a DNS-route-safe identity with nonblank
    /// access credentials.
    async fn create_sandbox(&self, request: ControlCreateSandbox) -> Result<ControlSandboxAccess>;
    /// Gets one sandbox and its provider state.
    async fn get_sandbox(&self, sandbox_id: &str) -> Result<ControlSandbox>;
    /// Gets read access only when the sandbox is already running and provider
    /// auto-resume is disabled; this operation never changes its timeout.
    async fn get_sandbox_read_access(&self, sandbox_id: &str) -> Result<ControlSandboxReadAccess>;
    /// Reacquires call-local credentials without extending a one-shot lifetime.
    ///
    /// Idle auto-pause and legacy sandboxes use provider connect. A running
    /// one-shot sandbox uses non-mutating read access and carries no traffic
    /// credential because E2B does not return one from sandbox detail.
    async fn connect_sandbox(&self, sandbox_id: &str) -> Result<ControlSandboxAccess>;
    /// Acquires credentials with a call-specific timeout for resumable sandboxes.
    /// One-shot sandboxes retain their original lifetime without reconnecting.
    async fn connect_sandbox_with_timeout(
        &self,
        sandbox_id: &str,
        timeout_seconds: u32,
    ) -> Result<ControlSandboxAccess>;
    /// Pauses an idle-auto-pause or legacy sandbox while retaining memory.
    /// One-shot metadata must be rejected without sending the mutation.
    async fn pause_sandbox(&self, sandbox_id: &str) -> Result<()>;
    /// Idempotently kills a sandbox.
    async fn kill_sandbox(&self, sandbox_id: &str) -> Result<()>;
    /// Creates one persistent snapshot with a nonempty recovery name and a
    /// route-safe returned provider identity.
    async fn create_snapshot(&self, sandbox_id: &str, name: &str) -> Result<ControlSnapshot>;
    /// Lists route-safe snapshots for one nonempty source sandbox and
    /// correlation name.
    async fn list_snapshots(&self, sandbox_id: &str, name: &str) -> Result<Vec<ControlSnapshot>>;
    /// Inspects the exact requested snapshot within source-and-name inventory.
    async fn get_snapshot(
        &self,
        sandbox_id: &str,
        name: &str,
        snapshot_id: &str,
    ) -> Result<ControlSnapshot>;
    /// Idempotently deletes one snapshot.
    async fn delete_snapshot(&self, snapshot_id: &str) -> Result<()>;
}

/// Shared E2B control API alias.
pub type DynE2bControlApi = Arc<dyn E2bControlApi>;
