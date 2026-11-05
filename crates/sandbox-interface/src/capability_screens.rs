//! Idempotent screen stack setup and exact viewport resizing.

use async_trait::async_trait;

use crate::{
    BackendEnsureScreenStackRequest, BackendResizeScreenStackRequest, Result, ScreenStackOutcome,
    ScreenViewportSize,
};

/// Idempotent screen stack setup and exact viewport resizing.
#[unimock::unimock(api = SandboxScreensMock)]
#[async_trait]
pub trait SandboxScreens: Send + Sync {
    /// Idempotently ensures a screen-capable sandbox's bridge stack.
    async fn ensure_screen_stack(
        &self,
        request: BackendEnsureScreenStackRequest,
    ) -> Result<ScreenStackOutcome>;
    /// Idempotently applies one exact viewport to a screen-capable sandbox.
    async fn resize_screen_stack(
        &self,
        request: BackendResizeScreenStackRequest,
    ) -> Result<ScreenViewportSize>;
}
