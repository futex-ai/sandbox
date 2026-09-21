//! Reusable backend conformance harness for provider implementations.

use std::time::Duration;

use uuid::Uuid;

use crate::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendInputRequest,
    BackendInspectSnapshotRequest, BackendOutputRequest, BackendPortIngressRequest,
    BackendReadFileRequest, BackendRealizeImageRequest, BackendRunProcessRequest,
    BackendSnapshotCreateOutcome, BackendTerminalCreateRequest, BackendWriteFileRequest, Error,
    OperationId, RealizeImageFileInput, ResourceOwner, Result, SandboxBackend, SandboxId,
    SandboxNetworkPolicy, SnapshotId, TerminalId,
};

/// Exercises the mandatory lifecycle shared by every sandbox backend.
pub async fn exercise_backend(backend: &dyn SandboxBackend, profile: &str) -> Result<()> {
    backend
        .list_managed_sandboxes("backend-conformance".to_owned())
        .await?;
    let workspace_id = Uuid::now_v7();
    let owner = ResourceOwner::agent(workspace_id, Uuid::now_v7());
    let source_request = sandbox_request(owner, profile, None);
    let source = backend.create_sandbox(source_request.clone()).await?;
    let recovered = backend
        .recover_sandbox_create(source_request)
        .await?
        .ok_or_else(|| Error::internal_message("backend did not recover a created sandbox"))?;
    if recovered.provider_ref != source.provider_ref {
        return Err(Error::internal_message(
            "backend sandbox recovery changed provider identity",
        ));
    }
    backend.inspect_sandbox(source.provider_ref.clone()).await?;
    let paused = backend.pause_sandbox(source.provider_ref.clone()).await?;
    backend.resume_sandbox(paused.provider_ref).await?;
    let ingress = backend
        .port_ingress(BackendPortIngressRequest {
            sandbox_provider_ref: source.provider_ref.clone(),
            port: 4173,
        })
        .await?;
    if ingress.upstream_url().is_empty() {
        return Err(Error::internal_message(
            "backend port ingress returned an empty upstream URL",
        ));
    }

    let correlation = format!("sandbox-conformance-{}", OperationId::new());
    let before = backend
        .snapshot_inventory(source.provider_ref.clone(), correlation.clone())
        .await?;
    let snapshot_request = BackendCreateSnapshotRequest {
        snapshot_id: SnapshotId::new(),
        operation_id: OperationId::new(),
        source_provider_ref: source.provider_ref.clone(),
        correlation_name: correlation,
        before,
    };
    let snapshot = backend.create_snapshot(snapshot_request.clone()).await?;
    let BackendSnapshotCreateOutcome::Created(snapshot) = snapshot else {
        return Err(Error::internal_message(
            "backend conformance snapshot did not complete",
        ));
    };
    let recovered_snapshot = backend
        .recover_snapshot_create(snapshot_request.clone())
        .await?;
    let crate::BackendSnapshotRecovery::Recovered(recovered_snapshot) = recovered_snapshot else {
        return Err(Error::internal_message(
            "backend did not recover a created snapshot",
        ));
    };
    if recovered_snapshot.provider_ref != snapshot.provider_ref {
        return Err(Error::internal_message(
            "backend snapshot recovery changed provider identity",
        ));
    }
    backend
        .inspect_snapshot(BackendInspectSnapshotRequest {
            provider_ref: snapshot.provider_ref.clone(),
            source_provider_ref: source.provider_ref.clone(),
            correlation_name: snapshot_request.correlation_name,
        })
        .await?;
    backend
        .write_file(BackendWriteFileRequest {
            sandbox_provider_ref: source.provider_ref.clone(),
            root: "/workspace".to_owned(),
            path: "conformance.txt".to_owned(),
            bytes: b"file-transfer".to_vec(),
        })
        .await?;
    let file = backend
        .read_file(BackendReadFileRequest {
            sandbox_provider_ref: source.provider_ref.clone(),
            root: "/workspace".to_owned(),
            path: "conformance.txt".to_owned(),
            offset: 0,
            max_bytes: 4096,
        })
        .await?;
    if file.bytes != b"file-transfer" || file.total_size != 13 {
        return Err(Error::internal_message(
            "backend file transfer changed regular-file content",
        ));
    }
    let process = backend
        .run_process(BackendRunProcessRequest {
            sandbox_provider_ref: source.provider_ref.clone(),
            command: "conformance-command".to_owned(),
            args: vec!["exact-argument".to_owned()],
            stdout_limit: 4096,
            stderr_limit: 1024,
            deadline: Duration::from_secs(60),
        })
        .await?;
    if process.stdout != b"argv-direct"
        || process.stderr != b"separate-stderr"
        || process.exit_code != Some(0)
        || !process.exited
        || process.stdout_overflowed
        || process.stderr_overflowed
    {
        return Err(Error::internal_message(
            "backend process run changed bounded split output",
        ));
    }

    let first = backend
        .create_sandbox(sandbox_request(
            owner,
            profile,
            Some(snapshot.provider_ref.clone()),
        ))
        .await?;
    backend
        .clean_restored_terminals(first.provider_ref.clone())
        .await?;
    let second = backend
        .create_sandbox(sandbox_request(
            owner,
            profile,
            Some(snapshot.provider_ref.clone()),
        ))
        .await?;
    backend
        .clean_restored_terminals(second.provider_ref.clone())
        .await?;
    if first.provider_ref == second.provider_ref {
        return Err(Error::internal_message(
            "backend conformance restores were not independent",
        ));
    }

    let terminal_request = BackendTerminalCreateRequest {
        terminal_id: TerminalId::new(),
        operation_id: OperationId::new(),
        sandbox_provider_ref: source.provider_ref.clone(),
        cwd: None,
        provider_log_limit: 2 * 1024 * 1024,
    };
    let terminal = backend.create_terminal(terminal_request.clone()).await?;
    let recovered_terminal = backend
        .recover_terminal_create(terminal_request)
        .await?
        .ok_or_else(|| Error::internal_message("backend did not recover a created terminal"))?;
    if recovered_terminal.provider_ref != terminal.provider_ref {
        return Err(Error::internal_message(
            "backend terminal recovery changed provider identity",
        ));
    }
    backend
        .inspect_terminal(source.provider_ref.clone(), terminal.provider_ref.clone())
        .await?;
    backend
        .write_terminal(BackendInputRequest {
            sandbox_provider_ref: source.provider_ref.clone(),
            terminal_provider_ref: terminal.provider_ref.clone(),
            input: b"printf 'conformance\\n'\n".to_vec(),
        })
        .await?;
    backend
        .read_terminal(BackendOutputRequest {
            sandbox_provider_ref: source.provider_ref.clone(),
            terminal_provider_ref: terminal.provider_ref.clone(),
            provider_log_path: terminal.provider_log_path,
            offset: 0,
            max_bytes: 4096,
            provider_log_limit: 2 * 1024 * 1024,
            wait: Duration::ZERO,
        })
        .await?;
    backend
        .close_terminal(source.provider_ref.clone(), terminal.provider_ref)
        .await?;

    let realized = backend
        .realize_image(BackendRealizeImageRequest {
            sandbox_id: SandboxId::new(),
            snapshot_id: SnapshotId::new(),
            operation_id: OperationId::new(),
            owner: ResourceOwner::platform(workspace_id),
            deployment_id: "backend-conformance".to_owned(),
            profile: profile.to_owned(),
            parent_image_provider_ref: None,
            input_files: vec![RealizeImageFileInput {
                root: "/tmp".to_owned(),
                path: "sandbox-conformance-input.bin".to_owned(),
                bytes: b"input".to_vec(),
            }],
            setup_script: "test -f /tmp/sandbox-conformance-input.bin".to_owned(),
            verify_commands: vec!["true".to_owned()],
            correlation_name: format!("sandbox-conformance-image-{}", OperationId::new()),
        })
        .await?;
    if realized.size_bytes == 0 {
        return Err(Error::internal_message(
            "backend image realization returned a zero size",
        ));
    }
    let failed_sandbox_id = SandboxId::new();
    let failure = backend
        .realize_image(BackendRealizeImageRequest {
            sandbox_id: failed_sandbox_id,
            snapshot_id: SnapshotId::new(),
            operation_id: OperationId::new(),
            owner: ResourceOwner::platform(workspace_id),
            deployment_id: "backend-conformance".to_owned(),
            profile: profile.to_owned(),
            parent_image_provider_ref: None,
            input_files: Vec::new(),
            setup_script: "exit 7".to_owned(),
            verify_commands: vec!["true".to_owned()],
            correlation_name: format!("sandbox-conformance-image-{}", OperationId::new()),
        })
        .await
        .expect_err("backend setup failure should be typed");
    match failure {
        Error::ImageSetupFailed {
            command: _,
            retained_sandbox: Some(retained_sandbox),
        } if retained_sandbox.sandbox_id == failed_sandbox_id => {
            backend
                .destroy_sandbox(retained_sandbox.provider_ref)
                .await?;
        }
        _ => {
            return Err(Error::internal_message(
                "backend setup failure did not retain its failed sandbox",
            ));
        }
    }
    backend.delete_snapshot(realized.image_provider_ref).await?;
    backend.delete_snapshot(snapshot.provider_ref).await?;
    backend.destroy_sandbox(first.provider_ref).await?;
    backend.destroy_sandbox(second.provider_ref).await?;
    backend.destroy_sandbox(source.provider_ref).await
}

fn sandbox_request(
    owner: ResourceOwner,
    profile: &str,
    snapshot_provider_ref: Option<crate::ProviderRef>,
) -> BackendCreateSandboxRequest {
    BackendCreateSandboxRequest {
        sandbox_id: SandboxId::new(),
        operation_id: OperationId::new(),
        owner,
        deployment_id: "backend-conformance".to_owned(),
        profile: profile.to_owned(),
        network: SandboxNetworkPolicy::Open,
        snapshot_provider_ref,
    }
}
