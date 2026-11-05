//! Core sandbox creation, recover-only reconciliation, inspection, and destruction.

use async_trait::async_trait;

use crate::{
    BackendCreateSandboxRequest, BackendManagedSandbox, BackendSandbox, ProviderRef, Result,
};

/// Core sandbox creation, recover-only reconciliation, inspection, and destruction.
#[unimock::unimock(api = SandboxLifecycleMock)]
#[async_trait]
pub trait SandboxLifecycle: Send + Sync {
    /// Lists provider sandboxes correlated to one consumer deployment.
    async fn list_managed_sandboxes(
        &self,
        deployment_id: String,
    ) -> Result<Vec<BackendManagedSandbox>>;
    /// Dispatches a new sandbox create after the caller records create intent.
    ///
    /// After local validation, the backend must send its one create mutation
    /// without a fallible provider inventory preflight. Ambiguous delivery may
    /// use recovery reads but must not dispatch a second mutation.
    /// Account capacity is advisory and cannot determine create success.
    /// A typed start refusal proves no work started or queued and needs no recovery.
    ///
    /// Once invocation starts, retries must call `recover_sandbox_create`
    /// instead of this method until the outcome is reconciled.
    async fn create_sandbox(&self, request: BackendCreateSandboxRequest) -> Result<BackendSandbox>;
    /// Reconciles a correlated sandbox create without dispatching a new create.
    ///
    /// Recovery preserves the original size, lifetime, and source selection.
    /// It revalidates the request's network policy and returns
    /// [`crate::Error::SandboxNetworkPolicyMismatch`] rather than adopting a
    /// correlated sandbox created under a different policy.
    async fn recover_sandbox_create(
        &self,
        request: BackendCreateSandboxRequest,
    ) -> Result<Option<BackendSandbox>>;
    /// Inspects one provider sandbox.
    async fn inspect_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox>;
    /// Idempotently destroys one provider sandbox.
    async fn destroy_sandbox(&self, provider_ref: ProviderRef) -> Result<()>;
}
