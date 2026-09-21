//! E2B backend construction and provider-neutral trait dispatch.

use std::sync::Arc;

use async_trait::async_trait;
use sandbox_interface::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendEnsureScreenStackRequest,
    BackendFileContent, BackendInputRequest, BackendInspectSnapshotRequest, BackendManagedSandbox,
    BackendOutputRequest, BackendPortIngressRequest, BackendReadFileRequest,
    BackendReadOnlyExecRequest, BackendRealizeImageRequest, BackendRealizedImage,
    BackendResizeScreenStackRequest, BackendRunProcessRequest, BackendSandbox, BackendSnapshot,
    BackendSnapshotCreateOutcome, BackendSnapshotInventory, BackendSnapshotRecovery,
    BackendTerminal, BackendTerminalCreateRequest, BackendTerminalOutput, BackendWriteFileRequest,
    DynSandboxBackend, PortIngress, ProviderRef, ReadOnlyExecOutput, Result, SandboxBackend,
    SandboxProcessOutput, ScreenStackOutcome, ScreenViewportSize,
};

use crate::{
    config::E2bAdapterConfig,
    control::{DynE2bControlApi, ReqwestE2bControlApi},
    process::{ConnectProcessTransport, DynProcessTransport},
};

use super::image_snapshot::{DynSnapshotRecoverySleeper, TokioSnapshotRecoverySleeper};

use super::{
    files, image_realization, mapping, port_ingress, process_run, read_only_exec, sandboxes,
    screen_resize, screen_stack, snapshots, terminal_output, terminals,
};

/// E2B implementation of the consumer's mandatory sandbox backend contract.
pub struct E2bSandboxBackend {
    pub(super) config: E2bAdapterConfig,
    pub(super) control: DynE2bControlApi,
    pub(super) processes: DynProcessTransport,
    pub(super) snapshot_recovery_sleeper: DynSnapshotRecoverySleeper,
}

impl E2bSandboxBackend {
    /// Builds a production E2B backend from validated adapter configuration.
    pub fn new(config: E2bAdapterConfig) -> Result<Self> {
        let control = mapping::control_result(
            ReqwestE2bControlApi::new(
                config.api_base.clone(),
                config.api_key.clone(),
                config.sandbox_domain.clone(),
                config.idle_timeout_seconds,
            ),
            &config.backend_id,
            None,
        )?;
        let processes = ConnectProcessTransport::new(config.backend_id.clone())?;
        Ok(Self {
            config,
            control: Arc::new(control),
            processes: Arc::new(processes),
            snapshot_recovery_sleeper: Arc::new(TokioSnapshotRecoverySleeper),
        })
    }

    /// Builds a backend with injected control and process transports.
    #[must_use]
    pub fn with_transports(
        config: E2bAdapterConfig,
        control: DynE2bControlApi,
        processes: DynProcessTransport,
    ) -> Self {
        Self {
            config,
            control,
            processes,
            snapshot_recovery_sleeper: Arc::new(TokioSnapshotRecoverySleeper),
        }
    }

    /// Converts this concrete adapter into the shared backend alias.
    #[must_use]
    pub fn into_backend(self) -> DynSandboxBackend {
        Arc::new(self)
    }
}

#[async_trait]
impl SandboxBackend for E2bSandboxBackend {
    async fn realize_image(
        &self,
        request: BackendRealizeImageRequest,
    ) -> Result<BackendRealizedImage> {
        image_realization::realize(self, request).await
    }

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

    async fn resume_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox> {
        sandboxes::resume(self, provider_ref).await
    }

    async fn port_ingress(&self, request: BackendPortIngressRequest) -> Result<PortIngress> {
        port_ingress::resolve(self, request).await
    }

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

    async fn pause_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox> {
        sandboxes::pause(self, provider_ref).await
    }

    async fn destroy_sandbox(&self, provider_ref: ProviderRef) -> Result<()> {
        sandboxes::destroy(self, provider_ref).await
    }

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

    async fn clean_restored_terminals(&self, sandbox_provider_ref: ProviderRef) -> Result<()> {
        terminals::clean_restored(self, sandbox_provider_ref).await
    }

    async fn run_process(&self, request: BackendRunProcessRequest) -> Result<SandboxProcessOutput> {
        process_run::run(self, request).await
    }

    async fn read_file(&self, request: BackendReadFileRequest) -> Result<BackendFileContent> {
        files::read(self, request).await
    }

    async fn read_only_exec(
        &self,
        request: BackendReadOnlyExecRequest,
    ) -> Result<ReadOnlyExecOutput> {
        read_only_exec::execute(self, request).await
    }

    async fn write_file(&self, request: BackendWriteFileRequest) -> Result<()> {
        files::write(self, request).await
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
        terminals::write(self, request).await
    }

    async fn close_terminal(
        &self,
        sandbox_provider_ref: ProviderRef,
        terminal_provider_ref: ProviderRef,
    ) -> Result<()> {
        terminals::close(self, sandbox_provider_ref, terminal_provider_ref).await
    }
}
