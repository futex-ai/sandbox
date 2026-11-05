//! Fresh durable consumer requests for each isolated test.

use std::time::Duration;

use sandbox_interface::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendSnapshotInventory,
    OperationId, ProviderRef, ResourceOwner, SandboxConsumer, SandboxId, SandboxLifetime,
    SandboxNetworkPolicy, SnapshotId,
};

use super::state::CAPTURE_SOURCE;

pub fn create() -> BackendCreateSandboxRequest {
    BackendCreateSandboxRequest {
        sandbox_id: SandboxId::new(),
        operation_id: OperationId::new(),
        owner: ResourceOwner::platform(SandboxId::new().as_uuid()),
        consumer: SandboxConsumer::Runtime,
        lifetime: SandboxLifetime::OneShot {
            max_lifetime: Duration::from_secs(21_600),
        },
        size: None,
        deployment_id: "fixture-deployment".to_owned(),
        profile: "general".to_owned(),
        network: SandboxNetworkPolicy::Open,
        snapshot_provider_ref: None,
    }
}

pub fn snapshot(name: &str) -> BackendCreateSnapshotRequest {
    BackendCreateSnapshotRequest {
        snapshot_id: SnapshotId::new(),
        operation_id: OperationId::new(),
        source_provider_ref: ProviderRef::new(CAPTURE_SOURCE),
        correlation_name: name.to_owned(),
        before: BackendSnapshotInventory::default(),
    }
}
