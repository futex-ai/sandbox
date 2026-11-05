//! Provider-neutral sandbox and snapshot request and result types.

use crate::{
    OperationId, ProviderRef, ResourceOwner, SandboxConsumer, SandboxId, SandboxLifetime,
    SandboxNetworkPolicy, SandboxSizeSelection, SandboxState, SnapshotId, SnapshotState,
};

/// Provider request to create a sandbox.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendCreateSandboxRequest {
    /// Stable consumer sandbox correlation handle.
    pub sandbox_id: SandboxId,
    /// Durable operation correlation handle.
    pub operation_id: OperationId,
    /// Opaque ownership metadata.
    pub owner: ResourceOwner,
    /// Substrate consumer class preserved in provider metadata.
    pub consumer: SandboxConsumer,
    /// Provider-neutral lifetime policy revalidated before provider dispatch.
    pub lifetime: SandboxLifetime,
    /// Optional provider size or minimum resources; absence preserves profile behavior.
    pub size: Option<SandboxSizeSelection>,
    /// Deployment identity used only as opaque metadata.
    pub deployment_id: String,
    /// Logical profile resolved by the adapter to provider configuration.
    pub profile: String,
    /// Typed per-session network policy revalidated before provider dispatch.
    pub network: SandboxNetworkPolicy,
    /// Provider snapshot ref for a restore, when applicable.
    pub snapshot_provider_ref: Option<ProviderRef>,
}

/// Provider sandbox state and opaque identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendSandbox {
    /// Opaque provider sandbox reference.
    pub provider_ref: ProviderRef,
    /// Mapped lifecycle state.
    pub state: SandboxState,
}

/// Provider sandbox discovered through deployment-scoped reconciliation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendManagedSandbox {
    /// Opaque provider sandbox reference.
    pub provider_ref: ProviderRef,
    /// Mapped provider lifecycle state.
    pub state: SandboxState,
    /// consumer sandbox handle recovered from opaque provider metadata.
    pub sandbox_id: Option<SandboxId>,
    /// Durable operation handle recovered from opaque provider metadata.
    pub operation_id: Option<OperationId>,
    /// Consumer class recovered from metadata, absent on legacy resources.
    pub consumer: Option<SandboxConsumer>,
    /// Lifetime recovered from metadata, absent when missing or malformed.
    pub lifetime: Option<SandboxLifetime>,
}

/// Provider snapshot state and opaque identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendSnapshot {
    /// Opaque provider snapshot reference.
    pub provider_ref: ProviderRef,
    /// Mapped lifecycle state.
    pub state: SnapshotState,
}

/// Source-and-correlation-filtered provider snapshot inventory.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BackendSnapshotInventory {
    /// Matching opaque snapshot refs in deterministic provider order.
    pub snapshots: Vec<ProviderRef>,
}

/// Provider request to inspect one snapshot within its durable source context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendInspectSnapshotRequest {
    /// Opaque provider snapshot reference.
    pub provider_ref: ProviderRef,
    /// Opaque provider source-sandbox reference.
    pub source_provider_ref: ProviderRef,
    /// Opaque provider correlation name recorded before snapshot dispatch.
    pub correlation_name: String,
}

/// Provider snapshot creation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendCreateSnapshotRequest {
    /// Stable consumer snapshot handle.
    pub snapshot_id: SnapshotId,
    /// Durable operation handle.
    pub operation_id: OperationId,
    /// Source provider sandbox reference.
    pub source_provider_ref: ProviderRef,
    /// Opaque provider correlation name derived from the operation ID.
    pub correlation_name: String,
    /// Inventory captured before dispatch.
    pub before: BackendSnapshotInventory,
}

/// Immediate provider result for snapshot creation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendSnapshotCreateOutcome {
    /// Provider returned one identifiable snapshot.
    ///
    /// A failed capture can retain this identity for cleanup. Callers must
    /// inspect its state before treating the capture as successful.
    Created(BackendSnapshot),
    /// Provider accepted work that remains in progress.
    InProgress,
    /// Transport outcome is ambiguous and requires inventory reconciliation.
    DeliveryAmbiguous,
}

/// At-most-once reconciliation result after ambiguous snapshot delivery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendSnapshotRecovery {
    /// Exactly one deterministic new provider candidate was proven.
    ///
    /// Identity proof does not make a `Failed` capture successful. Its handle
    /// remains available so the caller can delete the failed resource.
    Recovered(BackendSnapshot),
    /// The matching provider operation is still in progress.
    InProgress,
    /// Multiple candidates or another terminal conflict prevent safe adoption.
    ReconciliationRequired,
}
