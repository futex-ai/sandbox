//! Independent lifecycle and caller-owned-source snapshot conformance.

use uuid::Uuid;

use crate::{
    BackendCreateSnapshotRequest, BackendInspectSnapshotRequest, OperationId, ProviderRef,
    ResourceOwner, Result, SandboxLifecycle, SandboxLifetime, SandboxSnapshots, SnapshotId,
    conformance::sandbox_request,
    conformance_resources::{ConformanceResources, TokioRecoverySleeper, finish},
};

/// Proves managed inventory, create, recover-only identity, inspection, and destruction.
///
/// The caller selects a lifetime supported by the provider. This probe owns its
/// sandbox and cleans it after either success or failure.
pub async fn exercise_lifecycle(
    lifecycle: &dyn SandboxLifecycle,
    profile: &str,
    lifetime: SandboxLifetime,
) -> Result<()> {
    let sleeper = TokioRecoverySleeper;
    let mut resources = ConformanceResources::new(&sleeper);
    let outcome = async {
        lifecycle
            .list_managed_sandboxes("backend-conformance".to_owned())
            .await?;
        let sandbox = resources
            .create_sandbox(
                lifecycle,
                sandbox_request(
                    ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7()),
                    profile,
                    None,
                    lifetime,
                ),
            )
            .await?;
        lifecycle
            .inspect_sandbox(sandbox.provider_ref.clone())
            .await?;
        lifecycle.destroy_sandbox(sandbox.provider_ref).await
    }
    .await;
    let cleanup = resources.cleanup().await;
    finish(outcome, cleanup)
}

/// Proves saved snapshot creation, recover-only identity, inspection, and cleanup.
///
/// The caller owns the capture-enabled source. This probe never destroys that
/// source. It owns and deletes every snapshot that it dispatches.
pub async fn exercise_snapshots(
    snapshots: &dyn SandboxSnapshots,
    source_provider_ref: ProviderRef,
) -> Result<()> {
    let sleeper = TokioRecoverySleeper;
    let mut resources = ConformanceResources::new(&sleeper);
    let outcome = async {
        let correlation_name = format!("sandbox-conformance-{}", OperationId::new());
        let before = snapshots
            .snapshot_inventory(source_provider_ref.clone(), correlation_name.clone())
            .await?;
        let snapshot = resources
            .create_snapshot(
                snapshots,
                BackendCreateSnapshotRequest {
                    snapshot_id: SnapshotId::new(),
                    operation_id: OperationId::new(),
                    source_provider_ref: source_provider_ref.clone(),
                    correlation_name: correlation_name.clone(),
                    before,
                },
            )
            .await?;
        snapshots
            .inspect_snapshot(BackendInspectSnapshotRequest {
                provider_ref: snapshot.provider_ref,
                source_provider_ref,
                correlation_name,
            })
            .await?;
        Ok(())
    }
    .await;
    let cleanup = resources.cleanup().await;
    finish(outcome, cleanup)
}
