//! Dispatch for the three Boat capabilities supported by this crate.

use async_trait::async_trait;
use sandbox_interface::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendInspectSnapshotRequest,
    BackendManagedSandbox, BackendSandbox, BackendSnapshot, BackendSnapshotCreateOutcome,
    BackendSnapshotInventory, BackendSnapshotRecovery, ProviderRef, ResourceKind, Result,
    SandboxAccountCapacity, SandboxLifecycle, SandboxSizeOffer, SandboxSnapshots,
};

use super::{capacity, configured::BoatSandboxBackend, lifecycle, mapping, snapshots};

#[async_trait]
impl SandboxLifecycle for BoatSandboxBackend {
    async fn create_sandbox(&self, request: BackendCreateSandboxRequest) -> Result<BackendSandbox> {
        lifecycle::create(self, request).await
    }
    async fn recover_sandbox_create(
        &self,
        request: BackendCreateSandboxRequest,
    ) -> Result<Option<BackendSandbox>> {
        lifecycle::recover(self, request).await
    }
    async fn list_managed_sandboxes(
        &self,
        deployment_id: String,
    ) -> Result<Vec<BackendManagedSandbox>> {
        lifecycle::managed(self, deployment_id).await
    }
    async fn inspect_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox> {
        Ok(mapping::sandbox(mapping::control_result(
            self.control.get_sandbox(provider_ref.as_str()).await,
            self.config.backend_id(),
            Some(ResourceKind::Sandbox),
        )?))
    }
    async fn destroy_sandbox(&self, provider_ref: ProviderRef) -> Result<()> {
        mapping::control_result(
            self.control.delete_sandbox(provider_ref.as_str()).await,
            self.config.backend_id(),
            Some(ResourceKind::Sandbox),
        )
    }
}

#[async_trait]
impl SandboxSnapshots for BoatSandboxBackend {
    async fn snapshot_inventory(
        &self,
        source_provider_ref: ProviderRef,
        correlation_name: String,
    ) -> Result<BackendSnapshotInventory> {
        snapshots::inventory(self, source_provider_ref, correlation_name).await
    }
    async fn create_snapshot(
        &self,
        request: BackendCreateSnapshotRequest,
    ) -> Result<BackendSnapshotCreateOutcome> {
        snapshots::create(self, request).await
    }
    async fn recover_snapshot_create(
        &self,
        request: BackendCreateSnapshotRequest,
    ) -> Result<BackendSnapshotRecovery> {
        snapshots::recover(self, request).await
    }
    async fn inspect_snapshot(
        &self,
        request: BackendInspectSnapshotRequest,
    ) -> Result<BackendSnapshot> {
        snapshots::inspect(self, request).await
    }
    async fn delete_snapshot(&self, provider_ref: ProviderRef) -> Result<()> {
        mapping::control_result(
            self.control.delete_snapshot(provider_ref.as_str()).await,
            self.config.backend_id(),
            Some(ResourceKind::Snapshot),
        )
    }
}

#[async_trait]
impl SandboxAccountCapacity for BoatSandboxBackend {
    async fn account_capacity(&self) -> Result<Vec<SandboxSizeOffer>> {
        capacity::offers(self).await
    }
}
