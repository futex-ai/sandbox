//! Boat backend dispatch for the two implemented runtime capabilities.

use async_trait::async_trait;
use sandbox_interface::{
    BackendFileContent, BackendReadFileRequest, BackendReadOnlyExecRequest,
    BackendRunProcessRequest, BackendStreamProcessRequest, BackendWriteFileRequest,
    ProcessEventStream, ReadOnlyExecOutput, Result, SandboxFiles, SandboxProcessOutput,
    SandboxProcesses,
};

use crate::backend::configured::BoatSandboxBackend;

#[async_trait]
impl SandboxProcesses for BoatSandboxBackend {
    async fn run_process(&self, request: BackendRunProcessRequest) -> Result<SandboxProcessOutput> {
        self.runtime.run_process(request).await
    }
    async fn stream_process(
        &self,
        request: BackendStreamProcessRequest,
    ) -> Result<ProcessEventStream> {
        self.runtime.stream_process(request).await
    }
    async fn read_only_exec(
        &self,
        request: BackendReadOnlyExecRequest,
    ) -> Result<ReadOnlyExecOutput> {
        self.runtime.read_only_exec(request).await
    }
}
#[async_trait]
impl SandboxFiles for BoatSandboxBackend {
    async fn read_file(&self, request: BackendReadFileRequest) -> Result<BackendFileContent> {
        self.runtime.read_file(request).await
    }
    async fn write_file(&self, request: BackendWriteFileRequest) -> Result<()> {
        self.runtime.write_file(request).await
    }
}
