//! Fake alternate backend used by provider-neutral conformance coverage.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use sandbox_interface::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendInspectSnapshotRequest,
    BackendManagedSandbox, BackendPrepareImageRequest, BackendPreparedImage, BackendSandbox,
    BackendSnapshot, BackendSnapshotCreateOutcome, BackendSnapshotInventory,
    BackendSnapshotRecovery, Error, OperationId, ProviderRef, Result, SandboxImagePreparation,
    SandboxLifecycle, SandboxNetworkPolicy, SandboxResumableLifecycle, SandboxSnapshots,
    SandboxState, SnapshotState,
};

use sandbox_interface::conformance::BackendConformance;

use super::{alternate_backend_faults::AlternateBackendFaults, alternate_image_realization};

#[derive(Default)]
pub(super) struct AlternateBackend {
    next_sandbox: AtomicU64,
    next_snapshot: AtomicU64,
    sandboxes: Mutex<HashMap<OperationId, (ProviderRef, SandboxNetworkPolicy)>>,
    snapshots: Mutex<Vec<ProviderRef>>,
    pub(super) files: Mutex<HashMap<String, Vec<u8>>>,
    pub(super) faults: AlternateBackendFaults,
}

#[async_trait]
impl SandboxLifecycle for AlternateBackend {
    async fn list_managed_sandboxes(
        &self,
        _deployment_id: String,
    ) -> Result<Vec<BackendManagedSandbox>> {
        Ok(Vec::new())
    }

    async fn create_sandbox(&self, request: BackendCreateSandboxRequest) -> Result<BackendSandbox> {
        let network = request.network.validated()?;
        request.lifetime.validate()?;
        self.faults.record_sandbox_lifetime(request.lifetime);
        let index = self.next_sandbox.fetch_add(1, Ordering::Relaxed);
        let provider_ref = ProviderRef::new(format!("alternate-sandbox-{index}"));
        self.sandboxes
            .lock()
            .expect("sandbox lock")
            .insert(request.operation_id, (provider_ref.clone(), network));
        if self.faults.record_sandbox_create(&provider_ref) {
            return Err(Error::BackendUnavailable {
                backend_id: "alternate".to_owned(),
            });
        }
        Ok(BackendSandbox {
            provider_ref,
            state: SandboxState::Ready,
        })
    }

    async fn recover_sandbox_create(
        &self,
        request: BackendCreateSandboxRequest,
    ) -> Result<Option<BackendSandbox>> {
        let network = request.network.validated()?;
        request.lifetime.validate()?;
        if self.faults.miss_recovery() {
            return Ok(None);
        }
        let sandboxes = self.sandboxes.lock().expect("sandbox lock");
        let Some((provider_ref, created_network)) = sandboxes.get(&request.operation_id) else {
            return Ok(None);
        };
        if created_network != &network {
            return Err(Error::SandboxNetworkPolicyMismatch);
        }
        Ok(Some(BackendSandbox {
            provider_ref: provider_ref.clone(),
            state: SandboxState::Ready,
        }))
    }

    async fn inspect_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox> {
        Ok(BackendSandbox {
            provider_ref,
            state: SandboxState::Ready,
        })
    }

    async fn destroy_sandbox(&self, provider_ref: ProviderRef) -> Result<()> {
        self.faults.record_sandbox_destroy(&provider_ref);
        Ok(())
    }
}

#[async_trait]
impl SandboxResumableLifecycle for AlternateBackend {
    async fn resume_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox> {
        self.inspect_sandbox(provider_ref).await
    }

    async fn pause_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox> {
        Ok(BackendSandbox {
            provider_ref,
            state: SandboxState::Paused,
        })
    }
}

#[async_trait]
impl SandboxSnapshots for AlternateBackend {
    async fn snapshot_inventory(
        &self,
        _source_provider_ref: ProviderRef,
        _correlation_name: String,
    ) -> Result<BackendSnapshotInventory> {
        Ok(BackendSnapshotInventory {
            snapshots: self.snapshots.lock().expect("snapshot lock").clone(),
        })
    }

    async fn create_snapshot(
        &self,
        _request: BackendCreateSnapshotRequest,
    ) -> Result<BackendSnapshotCreateOutcome> {
        let index = self.next_snapshot.fetch_add(1, Ordering::Relaxed);
        let provider_ref = ProviderRef::new(format!("alternate-snapshot-{index}"));
        self.faults.record_snapshot_create(&provider_ref);
        self.snapshots
            .lock()
            .expect("snapshot lock")
            .push(provider_ref);
        Ok(BackendSnapshotCreateOutcome::InProgress)
    }

    async fn recover_snapshot_create(
        &self,
        request: BackendCreateSnapshotRequest,
    ) -> Result<BackendSnapshotRecovery> {
        let inventory = self
            .snapshot_inventory(request.source_provider_ref, request.correlation_name)
            .await?;
        let new = inventory
            .snapshots
            .into_iter()
            .filter(|candidate| !request.before.snapshots.contains(candidate))
            .collect::<Vec<_>>();
        match new.as_slice() {
            [provider_ref] => Ok(BackendSnapshotRecovery::Recovered(BackendSnapshot {
                provider_ref: provider_ref.clone(),
                state: SnapshotState::Ready,
            })),
            _ => Ok(BackendSnapshotRecovery::ReconciliationRequired),
        }
    }

    async fn inspect_snapshot(
        &self,
        request: BackendInspectSnapshotRequest,
    ) -> Result<BackendSnapshot> {
        Ok(BackendSnapshot {
            provider_ref: request.provider_ref,
            state: SnapshotState::Ready,
        })
    }

    async fn delete_snapshot(&self, provider_ref: ProviderRef) -> Result<()> {
        if self.faults.record_snapshot_delete(&provider_ref) {
            return Err(Error::BackendUnavailable {
                backend_id: "alternate".to_owned(),
            });
        }
        Ok(())
    }
}

#[async_trait]
impl SandboxImagePreparation for AlternateBackend {
    async fn prepare_image(
        &self,
        request: BackendPrepareImageRequest,
    ) -> Result<BackendPreparedImage> {
        alternate_image_realization::prepare(request)
    }
}

impl AlternateBackend {
    pub(super) fn capabilities(&self) -> BackendConformance<'_> {
        BackendConformance {
            lifecycle: self,
            resumable: self,
            snapshots: self,
            images: self,
            processes: self,
            files: self,
            terminals: self,
            ingress: self,
        }
    }
}
