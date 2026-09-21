//! Shared live-test configuration, polling, resource tracking, and cleanup.

use std::{collections::HashMap, error::Error, io, time::Duration};

use sandbox_e2b::{E2bAdapterConfig, E2bProfile, E2bSandboxBackend};
use sandbox_interface::{
    BackendCreateSandboxRequest, BackendOutputRequest, BackendTerminal,
    BackendTerminalCreateRequest, Error as SandboxError, OperationId, ProviderRef, ResourceOwner,
    SandboxBackend, SandboxId, TerminalId,
};

const LOG_LIMIT: usize = 2 * 1024 * 1024;

pub(super) type LiveResult<T> = Result<T, Box<dyn Error>>;

#[derive(Default)]
pub(super) struct LiveResources {
    pub(super) sandboxes: Vec<ProviderRef>,
    pub(super) terminals: Vec<(ProviderRef, ProviderRef)>,
    pub(super) snapshot: Option<ProviderRef>,
}

impl LiveResources {
    pub(super) fn track_terminal(&mut self, sandbox: &ProviderRef, terminal: &BackendTerminal) {
        self.terminals
            .push((sandbox.clone(), terminal.provider_ref.clone()));
    }

    pub(super) async fn cleanup(&self, backend: &E2bSandboxBackend) -> LiveResult<()> {
        let mut first_error = None;
        for (sandbox, terminal) in self.terminals.iter().rev() {
            if let Err(error) = backend
                .close_terminal(sandbox.clone(), terminal.clone())
                .await
                && !matches!(error, SandboxError::NotFound { .. })
            {
                first_error.get_or_insert(error);
            }
        }
        for sandbox in self.sandboxes.iter().rev() {
            if let Err(error) = backend.destroy_sandbox(sandbox.clone()).await
                && !matches!(error, SandboxError::NotFound { .. })
            {
                first_error.get_or_insert(error);
            }
        }
        if let Some(snapshot) = &self.snapshot
            && let Err(error) = backend.delete_snapshot(snapshot.clone()).await
            && !matches!(error, SandboxError::NotFound { .. })
        {
            first_error.get_or_insert(error);
        }
        match first_error {
            Some(error) => Err(Box::new(error)),
            None => Ok(()),
        }
    }
}

pub(super) fn live_config(api_key: String, template: String) -> E2bAdapterConfig {
    E2bAdapterConfig::new(
        "e2b-live",
        "https://api.e2b.app",
        api_key,
        HashMap::from([(
            "live".to_owned(),
            E2bProfile {
                template,
                allow_public_egress: false,
                denied_destinations: vec!["203.0.113.10/32".to_owned()],
            },
        )]),
        600,
    )
    .expect("live adapter config")
}

pub(super) fn sandbox_request(
    owner: ResourceOwner,
    snapshot_provider_ref: Option<ProviderRef>,
) -> BackendCreateSandboxRequest {
    BackendCreateSandboxRequest {
        network: sandbox_interface::SandboxNetworkPolicy::Open,
        sandbox_id: SandboxId::new(),
        operation_id: OperationId::new(),
        owner,
        deployment_id: "live-test".to_owned(),
        profile: "live".to_owned(),
        snapshot_provider_ref,
    }
}

pub(super) fn terminal_request(sandbox_provider_ref: ProviderRef) -> BackendTerminalCreateRequest {
    BackendTerminalCreateRequest {
        terminal_id: TerminalId::new(),
        operation_id: OperationId::new(),
        sandbox_provider_ref,
        cwd: None,
        provider_log_limit: LOG_LIMIT,
    }
}

pub(super) async fn wait_for_output(
    backend: &E2bSandboxBackend,
    sandbox: ProviderRef,
    terminal: &BackendTerminal,
    mut offset: u64,
    marker: &[u8],
) -> LiveResult<u64> {
    for _ in 0..40 {
        let output = backend
            .read_terminal(BackendOutputRequest {
                sandbox_provider_ref: sandbox.clone(),
                terminal_provider_ref: terminal.provider_ref.clone(),
                provider_log_path: terminal.provider_log_path.clone(),
                offset,
                max_bytes: 64 * 1024,
                provider_log_limit: LOG_LIMIT,
                wait: Duration::from_millis(250),
            })
            .await?;
        offset = output.next_offset;
        if output
            .bytes
            .windows(marker.len())
            .any(|window| window == marker)
        {
            return Ok(offset);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(io::Error::other("terminal marker was not observed").into())
}
