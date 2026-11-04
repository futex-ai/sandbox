//! Independent provider-neutral capability implementations for E2B.

use async_trait::async_trait;
use sandbox_interface::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendEnsureScreenStackRequest,
    BackendFileContent, BackendInputRequest, BackendInspectSnapshotRequest, BackendManagedSandbox,
    BackendOutputRequest, BackendPortIngressRequest, BackendPrepareImageRequest,
    BackendPreparedImage, BackendReadFileRequest, BackendReadOnlyExecRequest,
    BackendResizeScreenStackRequest, BackendRunProcessRequest, BackendSandbox, BackendSnapshot,
    BackendSnapshotCreateOutcome, BackendSnapshotInventory, BackendSnapshotRecovery,
    BackendStreamProcessRequest, BackendTerminal, BackendTerminalCreateRequest,
    BackendTerminalOutput, BackendWriteFileRequest, PortIngress, ProcessEventStream, ProviderRef,
    ReadOnlyExecOutput, Result, SandboxFiles, SandboxImagePreparation, SandboxLifecycle,
    SandboxPortIngress, SandboxProcessOutput, SandboxProcesses, SandboxResumableLifecycle,
    SandboxScreens, SandboxSnapshots, SandboxTerminals, ScreenStackOutcome, ScreenViewportSize,
};

use super::{
    configured::E2bSandboxBackend, files, image_realization, port_ingress, process_run,
    process_stream, read_only_exec, sandboxes, screen_resize, screen_stack, snapshots,
    terminal_operations, terminal_output, terminals,
};

#[async_trait]
impl SandboxLifecycle for E2bSandboxBackend {
    async fn list_managed_sandboxes(
        &self,
        deployment_id: String,
    ) -> Result<Vec<BackendManagedSandbox>> {
        sandboxes::managed(self, deployment_id).await
    }

    async fn create_sandbox(&self, request: BackendCreateSandboxRequest) -> Result<BackendSandbox> {
        sandboxes::create(self, request).await
    }

    async fn recover_sandbox_create(
        &self,
        request: BackendCreateSandboxRequest,
    ) -> Result<Option<BackendSandbox>> {
        sandboxes::recover(self, request).await
    }

    async fn inspect_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox> {
        sandboxes::inspect(self, provider_ref).await
    }

    async fn destroy_sandbox(&self, provider_ref: ProviderRef) -> Result<()> {
        sandboxes::destroy(self, provider_ref).await
    }
}

#[async_trait]
impl SandboxResumableLifecycle for E2bSandboxBackend {
    async fn resume_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox> {
        sandboxes::resume(self, provider_ref).await
    }

    async fn pause_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox> {
        sandboxes::pause(self, provider_ref).await
    }
}

#[async_trait]
impl SandboxSnapshots for E2bSandboxBackend {
    async fn snapshot_inventory(
        &self,
        source_provider_ref: ProviderRef,
        correlation_name: String,
    ) -> Result<BackendSnapshotInventory> {
        snapshots::inventory(self, source_provider_ref, correlation_name).await
    }

    async fn create_snapshot(
        &self,
        request: BackendCreateSnapshotRequest,
    ) -> Result<BackendSnapshotCreateOutcome> {
        snapshots::create(self, request).await
    }

    async fn recover_snapshot_create(
        &self,
        request: BackendCreateSnapshotRequest,
    ) -> Result<BackendSnapshotRecovery> {
        snapshots::recover(self, request).await
    }

    async fn inspect_snapshot(
        &self,
        request: BackendInspectSnapshotRequest,
    ) -> Result<BackendSnapshot> {
        snapshots::inspect(self, request).await
    }

    async fn delete_snapshot(&self, provider_ref: ProviderRef) -> Result<()> {
        snapshots::delete(self, provider_ref).await
    }
}

#[async_trait]
impl SandboxImagePreparation for E2bSandboxBackend {
    async fn prepare_image(
        &self,
        request: BackendPrepareImageRequest,
    ) -> Result<BackendPreparedImage> {
        image_realization::prepare(self, request).await
    }
}

#[async_trait]
impl SandboxProcesses for E2bSandboxBackend {
    async fn run_process(&self, request: BackendRunProcessRequest) -> Result<SandboxProcessOutput> {
        process_run::run(self, request).await
    }

    async fn stream_process(
        &self,
        request: BackendStreamProcessRequest,
    ) -> Result<ProcessEventStream> {
        process_stream::stream(self, request).await
    }

    async fn read_only_exec(
        &self,
        request: BackendReadOnlyExecRequest,
    ) -> Result<ReadOnlyExecOutput> {
        read_only_exec::execute(self, request).await
    }
}

#[async_trait]
impl SandboxFiles for E2bSandboxBackend {
    async fn read_file(&self, request: BackendReadFileRequest) -> Result<BackendFileContent> {
        files::read(self, request).await
    }

    async fn write_file(&self, request: BackendWriteFileRequest) -> Result<()> {
        files::write(self, request).await
    }
}

#[async_trait]
impl SandboxTerminals for E2bSandboxBackend {
    async fn clean_restored_terminals(&self, sandbox_provider_ref: ProviderRef) -> Result<()> {
        terminal_operations::clean_restored(self, sandbox_provider_ref).await
    }

    async fn create_terminal(
        &self,
        request: BackendTerminalCreateRequest,
    ) -> Result<BackendTerminal> {
        terminals::create(self, request).await
    }

    async fn recover_terminal_create(
        &self,
        request: BackendTerminalCreateRequest,
    ) -> Result<Option<BackendTerminal>> {
        terminals::recover(self, request).await
    }

    async fn inspect_terminal(
        &self,
        sandbox_provider_ref: ProviderRef,
        terminal_provider_ref: ProviderRef,
    ) -> Result<BackendTerminal> {
        terminals::inspect(self, sandbox_provider_ref, terminal_provider_ref).await
    }

    async fn read_terminal(&self, request: BackendOutputRequest) -> Result<BackendTerminalOutput> {
        terminal_output::read(self, request).await
    }

    async fn write_terminal(&self, request: BackendInputRequest) -> Result<()> {
        terminal_operations::write(self, request).await
    }

    async fn close_terminal(
        &self,
        sandbox_provider_ref: ProviderRef,
        terminal_provider_ref: ProviderRef,
    ) -> Result<()> {
        terminal_operations::close(self, sandbox_provider_ref, terminal_provider_ref).await
    }
}

#[async_trait]
impl SandboxPortIngress for E2bSandboxBackend {
    async fn port_ingress(&self, request: BackendPortIngressRequest) -> Result<PortIngress> {
        port_ingress::resolve(self, request).await
    }
}

#[async_trait]
impl SandboxScreens for E2bSandboxBackend {
    async fn ensure_screen_stack(
        &self,
        request: BackendEnsureScreenStackRequest,
    ) -> Result<ScreenStackOutcome> {
        screen_stack::ensure(self, request).await
    }

    async fn resize_screen_stack(
        &self,
        request: BackendResizeScreenStackRequest,
    ) -> Result<ScreenViewportSize> {
        screen_resize::resize(self, request).await
    }
}
