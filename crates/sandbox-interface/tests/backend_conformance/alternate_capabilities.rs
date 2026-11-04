//! Independent process, file, terminal, ingress, and screen capabilities for the test provider.

use async_trait::async_trait;
use sandbox_interface::{
    BackendEnsureScreenStackRequest, BackendFileContent, BackendInputRequest, BackendOutputRequest,
    BackendPortIngressRequest, BackendReadFileRequest, BackendReadOnlyExecRequest,
    BackendResizeScreenStackRequest, BackendRunProcessRequest, BackendStreamProcessRequest,
    BackendTerminal, BackendTerminalCreateRequest, BackendTerminalOutput, BackendWriteFileRequest,
    Error, PortIngress, ProcessEventStream, ProviderRef, ReadOnlyExecOutput, Result, SandboxFiles,
    SandboxPortIngress, SandboxProcessOutput, SandboxProcesses, SandboxScreens, SandboxTerminals,
    ScreenStackCapabilities, ScreenStackOutcome, ScreenViewportSize, TerminalState,
};

use super::{alternate_file, alternate_process, alternate_terminal};

use super::alternate_backend::AlternateBackend;

#[async_trait]
impl SandboxProcesses for AlternateBackend {
    async fn run_process(&self, request: BackendRunProcessRequest) -> Result<SandboxProcessOutput> {
        alternate_process::run(request)
    }

    async fn stream_process(
        &self,
        request: BackendStreamProcessRequest,
    ) -> Result<ProcessEventStream> {
        alternate_process::stream(request)
    }

    async fn read_only_exec(
        &self,
        request: BackendReadOnlyExecRequest,
    ) -> Result<ReadOnlyExecOutput> {
        Ok(ReadOnlyExecOutput {
            bytes: request.args.join("\0").into_bytes(),
            exit_code: 0,
        })
    }
}

#[async_trait]
impl SandboxFiles for AlternateBackend {
    async fn read_file(&self, request: BackendReadFileRequest) -> Result<BackendFileContent> {
        alternate_file::read(&self.files, request)
    }

    async fn write_file(&self, request: BackendWriteFileRequest) -> Result<()> {
        alternate_file::write(&self.files, request)
    }
}

#[async_trait]
impl SandboxTerminals for AlternateBackend {
    async fn clean_restored_terminals(&self, _sandbox_provider_ref: ProviderRef) -> Result<()> {
        Ok(())
    }

    async fn create_terminal(
        &self,
        request: BackendTerminalCreateRequest,
    ) -> Result<BackendTerminal> {
        let terminal =
            alternate_terminal::terminal(ProviderRef::new(request.terminal_id.to_string()));
        self.faults.record_terminal_create(&terminal.provider_ref);
        Ok(terminal)
    }

    async fn recover_terminal_create(
        &self,
        request: BackendTerminalCreateRequest,
    ) -> Result<Option<BackendTerminal>> {
        Ok(Some(alternate_terminal::terminal(ProviderRef::new(
            request.terminal_id.to_string(),
        ))))
    }

    async fn inspect_terminal(
        &self,
        _sandbox_provider_ref: ProviderRef,
        terminal_provider_ref: ProviderRef,
    ) -> Result<BackendTerminal> {
        Ok(alternate_terminal::terminal(terminal_provider_ref))
    }

    async fn read_terminal(&self, request: BackendOutputRequest) -> Result<BackendTerminalOutput> {
        Ok(BackendTerminalOutput {
            bytes: Vec::new(),
            next_offset: request.offset,
            total_size: request.offset,
            state: TerminalState::Ready,
            exit_code: None,
            overflowed: false,
        })
    }

    async fn write_terminal(&self, _request: BackendInputRequest) -> Result<()> {
        Ok(())
    }

    async fn close_terminal(
        &self,
        _sandbox_provider_ref: ProviderRef,
        terminal_provider_ref: ProviderRef,
    ) -> Result<()> {
        if self.faults.record_terminal_close(&terminal_provider_ref) {
            return Err(Error::BackendUnavailable {
                backend_id: "alternate".to_owned(),
            });
        }
        Ok(())
    }
}

#[async_trait]
impl SandboxPortIngress for AlternateBackend {
    async fn port_ingress(&self, request: BackendPortIngressRequest) -> Result<PortIngress> {
        Ok(PortIngress::new(
            format!(
                "https://{}-{}.alternate.invalid",
                request.port,
                request.sandbox_provider_ref.as_str()
            ),
            None,
        ))
    }
}

#[async_trait]
impl SandboxScreens for AlternateBackend {
    async fn ensure_screen_stack(
        &self,
        _request: BackendEnsureScreenStackRequest,
    ) -> Result<ScreenStackOutcome> {
        Ok(ScreenStackOutcome::Ready {
            capabilities: ScreenStackCapabilities::default(),
        })
    }

    async fn resize_screen_stack(
        &self,
        request: BackendResizeScreenStackRequest,
    ) -> Result<ScreenViewportSize> {
        Ok(request.viewport)
    }
}
