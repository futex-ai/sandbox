//! Durable terminal identity, bounded transcripts, input, and restored cleanup.

use async_trait::async_trait;

use crate::{
    BackendInputRequest, BackendOutputRequest, BackendTerminal, BackendTerminalCreateRequest,
    BackendTerminalOutput, ProviderRef, Result,
};

/// Durable terminal identity, bounded transcripts, input, and restored cleanup.
#[unimock::unimock(api = SandboxTerminalsMock)]
#[async_trait]
pub trait SandboxTerminals: Send + Sync {
    /// Removes inherited consumer terminal helpers from a restored sandbox.
    async fn clean_restored_terminals(&self, sandbox_provider_ref: ProviderRef) -> Result<()>;
    /// Creates one persistent provider PTY.
    ///
    /// Durable transcript capture must be active before a user login profile
    /// can run so startup output and exits cannot bypass terminal bookkeeping.
    /// The provider identity must also be durably recoverable before that shell
    /// can exit.
    /// The transcript limit must be validated before provider access and cannot
    /// exceed [`crate::FILE_TRANSFER_MAX_BYTES`].
    async fn create_terminal(
        &self,
        request: BackendTerminalCreateRequest,
    ) -> Result<BackendTerminal>;
    /// Recovers a correlated terminal create without allocating a new PTY,
    /// including the original provider reference in `Exited` state when only
    /// trusted identity state remains.
    ///
    /// The transcript limit has the same pre-provider-access bound as create.
    async fn recover_terminal_create(
        &self,
        request: BackendTerminalCreateRequest,
    ) -> Result<Option<BackendTerminal>>;
    /// Inspects one provider PTY, including a durably recorded exited PTY.
    async fn inspect_terminal(
        &self,
        sandbox_provider_ref: ProviderRef,
        terminal_provider_ref: ProviderRef,
    ) -> Result<BackendTerminal>;
    /// Ingests bytes from the durable provider-side terminal log.
    ///
    /// Provider-side identity reads must carry an earlier absolute completion
    /// deadline that reserves termination and return time inside this
    /// operation's bounded wait.
    async fn read_terminal(&self, request: BackendOutputRequest) -> Result<BackendTerminalOutput>;
    /// Sends exact input once; success requires a decoded provider
    /// acknowledgment, and callers fail closed after ambiguous delivery.
    async fn write_terminal(&self, request: BackendInputRequest) -> Result<()>;
    /// Idempotently closes one provider PTY while retaining durable identity
    /// for final transcript reads.
    async fn close_terminal(
        &self,
        sandbox_provider_ref: ProviderRef,
        terminal_provider_ref: ProviderRef,
    ) -> Result<()>;
}
