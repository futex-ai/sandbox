//! Fake alternate backend used by provider-neutral conformance coverage.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use sandbox_interface::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendEnsureScreenStackRequest,
    BackendFileContent, BackendInputRequest, BackendInspectSnapshotRequest, BackendManagedSandbox,
    BackendOutputRequest, BackendPortIngressRequest, BackendPrepareImageRequest,
    BackendPreparedImage, BackendReadFileRequest, BackendReadOnlyExecRequest,
    BackendResizeScreenStackRequest, BackendRunProcessRequest, BackendSandbox, BackendSnapshot,
    BackendSnapshotCreateOutcome, BackendSnapshotInventory, BackendSnapshotRecovery,
    BackendTerminal, BackendTerminalCreateRequest, BackendTerminalOutput, BackendWriteFileRequest,
    OperationId, PortIngress, ProviderRef, ReadOnlyExecOutput, Result, SandboxBackend,
    SandboxProcessOutput, SandboxState, ScreenStackCapabilities, ScreenStackOutcome,
    ScreenViewportSize, SnapshotState, TerminalState,
};

use super::{alternate_image_realization, alternate_process};

#[derive(Default)]
pub(super) struct AlternateBackend {
    next_sandbox: AtomicU64,
    next_snapshot: AtomicU64,
    sandboxes: Mutex<HashMap<OperationId, ProviderRef>>,
    snapshots: Mutex<Vec<ProviderRef>>,
    files: Mutex<HashMap<String, Vec<u8>>>,
}

#[async_trait]
impl SandboxBackend for AlternateBackend {
    async fn prepare_image(
        &self,
        request: BackendPrepareImageRequest,
    ) -> Result<BackendPreparedImage> {
        alternate_image_realization::prepare(request)
    }

    async fn list_managed_sandboxes(
        &self,
        _deployment_id: String,
    ) -> Result<Vec<BackendManagedSandbox>> {
        Ok(Vec::new())
    }

    async fn create_sandbox(&self, request: BackendCreateSandboxRequest) -> Result<BackendSandbox> {
        let index = self.next_sandbox.fetch_add(1, Ordering::Relaxed);
        let provider_ref = ProviderRef::new(format!("alternate-sandbox-{index}"));
        self.sandboxes
            .lock()
            .expect("sandbox lock")
            .insert(request.operation_id, provider_ref.clone());
        Ok(BackendSandbox {
            provider_ref,
            state: SandboxState::Ready,
        })
    }

    async fn recover_sandbox_create(
        &self,
        request: BackendCreateSandboxRequest,
    ) -> Result<Option<BackendSandbox>> {
        Ok(self
            .sandboxes
            .lock()
            .expect("sandbox lock")
            .get(&request.operation_id)
            .cloned()
            .map(|provider_ref| BackendSandbox {
                provider_ref,
                state: SandboxState::Ready,
            }))
    }

    async fn inspect_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox> {
        Ok(BackendSandbox {
            provider_ref,
            state: SandboxState::Ready,
        })
    }

    async fn resume_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox> {
        self.inspect_sandbox(provider_ref).await
    }

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

    async fn pause_sandbox(&self, provider_ref: ProviderRef) -> Result<BackendSandbox> {
        Ok(BackendSandbox {
            provider_ref,
            state: SandboxState::Paused,
        })
    }

    async fn destroy_sandbox(&self, _provider_ref: ProviderRef) -> Result<()> {
        Ok(())
    }

    async fn snapshot_inventory(
        &self,
        _source_provider_ref: ProviderRef,
        _correlation_name: String,
    ) -> Result<BackendSnapshotInventory> {
        Ok(BackendSnapshotInventory {
            snapshots: self.snapshots.lock().expect("snapshot lock").clone(),
        })
    }

    async fn create_snapshot(
        &self,
        _request: BackendCreateSnapshotRequest,
    ) -> Result<BackendSnapshotCreateOutcome> {
        let index = self.next_snapshot.fetch_add(1, Ordering::Relaxed);
        let provider_ref = ProviderRef::new(format!("alternate-snapshot-{index}"));
        self.snapshots
            .lock()
            .expect("snapshot lock")
            .push(provider_ref.clone());
        Ok(BackendSnapshotCreateOutcome::Created(BackendSnapshot {
            provider_ref,
            state: SnapshotState::Ready,
        }))
    }

    async fn recover_snapshot_create(
        &self,
        request: BackendCreateSnapshotRequest,
    ) -> Result<BackendSnapshotRecovery> {
        let inventory = self
            .snapshot_inventory(request.source_provider_ref, request.correlation_name)
            .await?;
        let new = inventory
            .snapshots
            .into_iter()
            .filter(|candidate| !request.before.snapshots.contains(candidate))
            .collect::<Vec<_>>();
        match new.as_slice() {
            [provider_ref] => Ok(BackendSnapshotRecovery::Recovered(BackendSnapshot {
                provider_ref: provider_ref.clone(),
                state: SnapshotState::Ready,
            })),
            _ => Ok(BackendSnapshotRecovery::ReconciliationRequired),
        }
    }

    async fn inspect_snapshot(
        &self,
        request: BackendInspectSnapshotRequest,
    ) -> Result<BackendSnapshot> {
        Ok(BackendSnapshot {
            provider_ref: request.provider_ref,
            state: SnapshotState::Ready,
        })
    }

    async fn delete_snapshot(&self, _provider_ref: ProviderRef) -> Result<()> {
        Ok(())
    }

    async fn clean_restored_terminals(&self, _sandbox_provider_ref: ProviderRef) -> Result<()> {
        Ok(())
    }

    async fn run_process(&self, request: BackendRunProcessRequest) -> Result<SandboxProcessOutput> {
        alternate_process::run(request)
    }

    async fn read_file(&self, request: BackendReadFileRequest) -> Result<BackendFileContent> {
        let bytes = self
            .files
            .lock()
            .expect("file lock")
            .get(&request.path)
            .cloned()
            .unwrap_or_default();
        let total_size = u64::try_from(bytes.len()).expect("file size");
        let offset = usize::try_from(request.offset).expect("file offset");
        let bytes = bytes
            .get(offset..)
            .unwrap_or_default()
            .iter()
            .copied()
            .take(request.max_bytes)
            .collect();
        Ok(BackendFileContent { bytes, total_size })
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

    async fn write_file(&self, request: BackendWriteFileRequest) -> Result<()> {
        self.files
            .lock()
            .expect("file lock")
            .insert(request.path, request.bytes);
        Ok(())
    }

    async fn create_terminal(
        &self,
        request: BackendTerminalCreateRequest,
    ) -> Result<BackendTerminal> {
        Ok(BackendTerminal {
            provider_ref: ProviderRef::new(request.terminal_id.to_string()),
            provider_log_path: "/tmp/alternate.log".to_owned(),
            state: TerminalState::Ready,
        })
    }

    async fn recover_terminal_create(
        &self,
        request: BackendTerminalCreateRequest,
    ) -> Result<Option<BackendTerminal>> {
        Ok(Some(BackendTerminal {
            provider_ref: ProviderRef::new(request.terminal_id.to_string()),
            provider_log_path: "/tmp/alternate.log".to_owned(),
            state: TerminalState::Ready,
        }))
    }

    async fn inspect_terminal(
        &self,
        _sandbox_provider_ref: ProviderRef,
        terminal_provider_ref: ProviderRef,
    ) -> Result<BackendTerminal> {
        Ok(BackendTerminal {
            provider_ref: terminal_provider_ref,
            provider_log_path: "/tmp/alternate.log".to_owned(),
            state: TerminalState::Ready,
        })
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
        _terminal_provider_ref: ProviderRef,
    ) -> Result<()> {
        Ok(())
    }
}
