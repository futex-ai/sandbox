//! Caller-owned image source preparation without resource allocation or deletion.

use async_trait::async_trait;

use crate::{BackendPrepareImageRequest, BackendPreparedImage, Result};

/// Caller-owned image source preparation without resource allocation or deletion.
#[unimock::unimock(api = SandboxImagePreparationMock)]
#[async_trait]
pub trait SandboxImagePreparation: Send + Sync {
    /// Prepares one durably recorded platform image source for snapshot dispatch.
    ///
    /// The caller creates or recovers the source in separate durable phases,
    /// invokes this preparation phase at most once, persists its result, then
    /// uses the snapshot inventory, create, and recovery methods below. This
    /// method never allocates, snapshots, or destroys a provider resource.
    /// Provider-owned safety phases must not load user-controlled shell startup
    /// files before they run.
    async fn prepare_image(
        &self,
        request: BackendPrepareImageRequest,
    ) -> Result<BackendPreparedImage>;
}
