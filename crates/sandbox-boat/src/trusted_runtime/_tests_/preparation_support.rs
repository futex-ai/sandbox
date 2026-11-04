//! Deterministic helper delivery gates around unimock expectations.

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use sandbox_interface::{BackendStreamProcessRequest, ProviderRef};
use tokio::sync::Notify;

use crate::{
    config::{BoatAdapterConfig, BoatProfile},
    error::Result,
    runtime::api::{AttachRequest, BoatRuntimeApi, DetachedProcess, HelperOutput},
    transport::runtime::BoatByteStream,
};

pub(super) struct GatedHelper {
    pub(super) inner: Arc<dyn BoatRuntimeApi>,
    pub(super) entered: Arc<Notify>,
    pub(super) release: Notify,
    pub(super) first: AtomicBool,
}

#[async_trait]
impl BoatRuntimeApi for GatedHelper {
    async fn helper(
        &self,
        sandbox: &str,
        command: String,
        timeout: Duration,
    ) -> Result<HelperOutput> {
        let output = self.inner.helper(sandbox, command, timeout).await?;
        if self.first.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        Ok(output)
    }
    async fn start(
        &self,
        sandbox: &str,
        command: String,
        timeout: Duration,
    ) -> Result<DetachedProcess> {
        self.inner.start(sandbox, command, timeout).await
    }
    async fn attach(&self, request: AttachRequest) -> Result<BoatByteStream> {
        self.inner.attach(request).await
    }
    async fn upload(
        &self,
        sandbox: &str,
        path: String,
        bytes: Vec<u8>,
        timeout: Duration,
    ) -> Result<()> {
        self.inner.upload(sandbox, path, bytes, timeout).await
    }
    async fn download(&self, sandbox: &str, path: String, timeout: Duration) -> Result<Vec<u8>> {
        self.inner.download(sandbox, path, timeout).await
    }
    async fn stop(&self, sandbox: &str, process: u64, timeout: Duration) -> Result<()> {
        self.inner.stop(sandbox, process, timeout).await
    }
}

pub(super) fn config() -> BoatAdapterConfig {
    BoatAdapterConfig::new(
        "boat",
        "https://boat.dev",
        "key",
        BTreeMap::from([(
            "general".to_owned(),
            BoatProfile {
                provider_size: "default".to_owned(),
                allow_public_egress: true,
                denied_destinations: Vec::new(),
            },
        )]),
    )
    .unwrap()
}

pub(super) fn request() -> BackendStreamProcessRequest {
    BackendStreamProcessRequest {
        sandbox_provider_ref: ProviderRef::new("bx_abcdefgh"),
        command: "/bin/true".to_owned(),
        args: Vec::new(),
        cwd: None,
        envs: BTreeMap::new(),
        stdout_limit: 32,
        stderr_limit: 32,
        deadline: Duration::from_secs(10),
        idle_timeout: Duration::from_secs(1),
    }
}
