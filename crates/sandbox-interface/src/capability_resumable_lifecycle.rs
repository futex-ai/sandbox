//! Optional pause and resume operations for resumable sandboxes.

use async_trait::async_trait;

use crate::{BackendSandbox, ProviderRef, Result};

/// Optional pause and resume operations for resumable sandboxes.
#[unimock::unimock(api = SandboxResumableLifecycleMock)]
#[async_trait]
pub trait SandboxResumableLifecycle: Send + Sync {
    /// Resumes or reconnects one provider sandbox.
    ///
    /// A one-shot sandbox may be verified only while already running; this
    /// operation must not resume it or extend its original maximum lifetime.
    async fn resume_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox>;
    /// Pauses one provider sandbox when supported; one-shot sandboxes reject it.
    async fn pause_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox>;
}
