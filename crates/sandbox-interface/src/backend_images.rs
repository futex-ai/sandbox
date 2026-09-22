//! Provider-neutral image-preparation backend contract.

use crate::{ProviderRef, RealizeImageFileInput, ResourceOwner, SandboxId};

/// Provider request to prepare one caller-owned source for snapshot dispatch.
///
/// The caller must durably persist `source_provider_ref` before starting this
/// one-shot phase. This operation never creates or destroys a sandbox and never
/// inventories, creates, or recovers a snapshot. Provider-owned verification,
/// scrub, and measurement must not load user-controlled login profiles.
#[derive(Clone, Eq, PartialEq)]
pub struct BackendPrepareImageRequest {
    /// Stable consumer sandbox handle for the persisted source runtime.
    pub sandbox_id: SandboxId,
    /// Persisted provider source that preparation may mutate but never destroy.
    pub source_provider_ref: ProviderRef,
    /// Workspace-platform owner required for this trusted preparation phase.
    pub owner: ResourceOwner,
    /// Credential-free files staged through bounded transfer before setup.
    pub input_files: Vec<RealizeImageFileInput>,
    /// Credential-free setup script.
    pub setup_script: String,
    /// Ordered commands that must all exit successfully.
    pub verify_commands: Vec<String>,
}

/// Provider result after preparing a source and before snapshot dispatch.
///
/// The caller must durably persist this result before recording snapshot
/// dispatch intent. It must then use the separate snapshot create and recovery
/// methods so a retry can never replay preparation or redispatch a snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendPreparedImage {
    /// Provider identity of the prepared caller-owned source runtime.
    pub source_provider_ref: ProviderRef,
    /// Observed retained image size in bytes.
    pub size_bytes: u64,
}
