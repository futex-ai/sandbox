//! Trusted runtime composition and capability orchestration.

use std::sync::Arc;

use async_trait::async_trait;
use sandbox_interface::{
    BackendFileContent, BackendReadFileRequest, BackendReadOnlyExecRequest,
    BackendRunProcessRequest, BackendStreamProcessRequest, BackendWriteFileRequest,
    ProcessEventStream, ReadOnlyExecOutput, Result, SandboxProcessOutput,
};

use crate::{config::BoatAdapterConfig, files, process};

use super::api::{BoatRuntimeApi, HelperOutput};

/// Supported runtime operations, independent from lifecycle mutation.
#[unimock::unimock(api = BoatRuntimeMock)]
#[async_trait]
pub trait BoatRuntime: Send + Sync {
    /// Executes a collected direct-argv process.
    async fn run_process(&self, request: BackendRunProcessRequest) -> Result<SandboxProcessOutput>;
    /// Executes an incremental direct-argv process.
    async fn stream_process(
        &self,
        request: BackendStreamProcessRequest,
    ) -> Result<ProcessEventStream>;
    /// Executes stateless combined-output argv without PTY allocation.
    async fn read_only_exec(
        &self,
        request: BackendReadOnlyExecRequest,
    ) -> Result<ReadOnlyExecOutput>;
    /// Reads through one confined regular-file descriptor.
    async fn read_file(&self, request: BackendReadFileRequest) -> Result<BackendFileContent>;
    /// Performs verified atomic replacement with trusted uncertainty fencing.
    async fn write_file(&self, request: BackendWriteFileRequest) -> Result<()>;
}

/// Trusted runtime implemented over Boat's documented HTTPS API.
pub struct BoatTrustedRuntime {
    pub(crate) config: BoatAdapterConfig,
    pub(crate) api: Arc<dyn BoatRuntimeApi>,
}

impl BoatTrustedRuntime {
    /// Composes a runtime without making provider calls.
    #[must_use]
    pub fn new(config: BoatAdapterConfig, api: Arc<dyn BoatRuntimeApi>) -> Self {
        Self { config, api }
    }
    pub(crate) fn unavailable(&self) -> sandbox_interface::Error {
        sandbox_interface::Error::BackendUnavailable {
            backend_id: self.config.backend_id().to_owned(),
        }
    }
    pub(crate) fn helper_succeeded(output: &HelperOutput) -> bool {
        output.normal && output.exit_code == Some(0) && output.stdout.is_empty()
    }
}

#[async_trait]
impl BoatRuntime for BoatTrustedRuntime {
    async fn run_process(&self, request: BackendRunProcessRequest) -> Result<SandboxProcessOutput> {
        process::collected::run(self, request).await
    }
    async fn stream_process(
        &self,
        request: BackendStreamProcessRequest,
    ) -> Result<ProcessEventStream> {
        process::execution::stream(self, request, None).await
    }
    async fn read_only_exec(
        &self,
        request: BackendReadOnlyExecRequest,
    ) -> Result<ReadOnlyExecOutput> {
        process::collected::read_only(self, request).await
    }
    async fn read_file(&self, request: BackendReadFileRequest) -> Result<BackendFileContent> {
        files::read::read(self, request).await
    }
    async fn write_file(&self, request: BackendWriteFileRequest) -> Result<()> {
        files::write::write(self, request).await
    }
}
