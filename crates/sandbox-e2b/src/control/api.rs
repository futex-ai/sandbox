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
    /// Lists route-safe running sandboxes matching exact consumer metadata.
    async fn list_sandboxes(&self, metadata: SandboxMetadata) -> Result<Vec<ControlSandbox>>;
    /// Creates one sandbox and returns a route-safe identity with nonblank
    /// access credentials.
    async fn create_sandbox(&self, request: ControlCreateSandbox) -> Result<ControlSandboxAccess>;
    /// Gets one sandbox and its provider state.
    async fn get_sandbox(&self, sandbox_id: &str) -> Result<ControlSandbox>;
    /// Gets read access only when the sandbox is already running and provider
    /// auto-resume is disabled; this operation never changes its timeout.
    async fn get_sandbox_read_access(&self, sandbox_id: &str) -> Result<ControlSandboxReadAccess>;
    /// Connects to or resumes a sandbox and reacquires nonblank call-local
    /// credentials.
    async fn connect_sandbox(&self, sandbox_id: &str) -> Result<ControlSandboxAccess>;
    /// Pauses a sandbox while retaining memory.
    async fn pause_sandbox(&self, sandbox_id: &str) -> Result<()>;
    /// Idempotently kills a sandbox.
    async fn kill_sandbox(&self, sandbox_id: &str) -> Result<()>;
    /// Creates one persistent snapshot with a nonempty recovery name and a
    /// route-safe returned provider identity.
    async fn create_snapshot(&self, sandbox_id: &str, name: &str) -> Result<ControlSnapshot>;
    /// Lists route-safe snapshots for one nonempty source sandbox and
    /// correlation name.
    async fn list_snapshots(&self, sandbox_id: &str, name: &str) -> Result<Vec<ControlSnapshot>>;
    /// Inspects a snapshot within its source-and-name-filtered inventory.
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
