//! Saved snapshot creation, recover-only reconciliation, inspection, and deletion.

use async_trait::async_trait;

use crate::{
    BackendCreateSnapshotRequest, BackendInspectSnapshotRequest, BackendSnapshot,
    BackendSnapshotCreateOutcome, BackendSnapshotInventory, BackendSnapshotRecovery, ProviderRef,
    Result,
};

/// Saved snapshot creation, recover-only reconciliation, inspection, and deletion.
#[unimock::unimock(api = SandboxSnapshotsMock)]
#[async_trait]
pub trait SandboxSnapshots: Send + Sync {
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
}
