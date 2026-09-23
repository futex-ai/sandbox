//! A virtual connection delay around a mocked control API.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;

use crate::error::Result;
use crate::{
    ControlCreateSandbox, ControlSandbox, ControlSandboxAccess, ControlSandboxReadAccess,
    ControlSnapshot, E2bControlApi, SandboxMetadata,
};

pub(super) struct DelayedControl {
    pub(super) inner: Arc<dyn E2bControlApi>,
    pub(super) latency: Duration,
}

#[async_trait]
impl E2bControlApi for DelayedControl {
    async fn connect_sandbox_with_timeout(
        &self,
        sandbox_id: &str,
        timeout_seconds: u32,
    ) -> Result<ControlSandboxAccess> {
        let result = self
            .inner
            .connect_sandbox_with_timeout(sandbox_id, timeout_seconds)
            .await;
        tokio::time::sleep(self.latency).await;
        result
    }

    async fn list_sandboxes(&self, metadata: SandboxMetadata) -> Result<Vec<ControlSandbox>> {
        self.inner.list_sandboxes(metadata).await
    }

    async fn create_sandbox(&self, request: ControlCreateSandbox) -> Result<ControlSandboxAccess> {
        self.inner.create_sandbox(request).await
    }

    async fn get_sandbox(&self, sandbox_id: &str) -> Result<ControlSandbox> {
        self.inner.get_sandbox(sandbox_id).await
    }

    async fn get_sandbox_read_access(&self, sandbox_id: &str) -> Result<ControlSandboxReadAccess> {
        self.inner.get_sandbox_read_access(sandbox_id).await
    }

    async fn connect_sandbox(&self, sandbox_id: &str) -> Result<ControlSandboxAccess> {
        self.inner.connect_sandbox(sandbox_id).await
    }

    async fn pause_sandbox(&self, sandbox_id: &str) -> Result<()> {
        self.inner.pause_sandbox(sandbox_id).await
    }

    async fn kill_sandbox(&self, sandbox_id: &str) -> Result<()> {
        self.inner.kill_sandbox(sandbox_id).await
    }

    async fn create_snapshot(&self, sandbox_id: &str, name: &str) -> Result<ControlSnapshot> {
        self.inner.create_snapshot(sandbox_id, name).await
    }

    async fn list_snapshots(&self, sandbox_id: &str, name: &str) -> Result<Vec<ControlSnapshot>> {
        self.inner.list_snapshots(sandbox_id, name).await
    }

    async fn get_snapshot(
        &self,
        sandbox_id: &str,
        name: &str,
        snapshot_id: &str,
    ) -> Result<ControlSnapshot> {
        self.inner.get_snapshot(sandbox_id, name, snapshot_id).await
    }

    async fn delete_snapshot(&self, snapshot_id: &str) -> Result<()> {
        self.inner.delete_snapshot(snapshot_id).await
    }
}
