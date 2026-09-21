//! Provider-neutral image-realization backend contract.

use crate::{
    OperationId, ProviderRef, RealizeImageFileInput, ResourceOwner, SandboxId, SnapshotId,
};

/// Provider request to realize one reusable workspace-platform image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendRealizeImageRequest {
    /// Stable consumer sandbox correlation handle for the ephemeral build runtime.
    pub sandbox_id: SandboxId,
    /// Stable consumer snapshot handle for the resulting image.
    pub snapshot_id: SnapshotId,
    /// Durable operation correlation handle.
    pub operation_id: OperationId,
    /// Workspace-platform owner encoded as opaque provider metadata.
    pub owner: ResourceOwner,
    /// Deployment identity used only as opaque provider metadata.
    pub deployment_id: String,
    /// Logical profile controlling provider configuration.
    pub profile: String,
    /// Parent provider image, or the profile template when absent.
    pub parent_image_provider_ref: Option<ProviderRef>,
    /// Credential-free files staged through bounded transfer before setup.
    pub input_files: Vec<RealizeImageFileInput>,
    /// Credential-free setup script.
    pub setup_script: String,
    /// Ordered commands that must all exit successfully.
    pub verify_commands: Vec<String>,
    /// Opaque provider snapshot correlation name.
    pub correlation_name: String,
}

/// Provider result after image realization and build-sandbox destruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendRealizedImage {
    /// Provider identity of the destroyed ephemeral build sandbox.
    pub source_sandbox_provider_ref: ProviderRef,
    /// Provider identity of the retained image.
    pub image_provider_ref: ProviderRef,
    /// Observed retained image size in bytes.
    pub size_bytes: u64,
}
