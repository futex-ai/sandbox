//! Reusable backend conformance harness for provider implementations.

use std::time::Duration;

use uuid::Uuid;

use crate::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendInputRequest,
    BackendInspectSnapshotRequest, BackendOutputRequest, BackendPortIngressRequest,
    BackendReadFileRequest, BackendRunProcessRequest, BackendTerminalCreateRequest,
    BackendWriteFileRequest, Error, OperationId, ResourceOwner, Result, SandboxBackend,
    SandboxConsumer, SandboxId, SandboxNetworkPolicy, SnapshotId, TerminalId,
    conformance_resources::{ConformanceResources, TokioRecoverySleeper, finish},
};

const PROCESS_SCRIPT: &str = "printf '%s' 'argv-direct'; printf '%s' 'separate-stderr' >&2";

/// Exercises the mandatory lifecycle shared by every sandbox backend.
///
/// The target image must provide `/bin/sh`; the split-output probe supplies its
/// own shell script.
pub async fn exercise_backend(backend: &dyn SandboxBackend, profile: &str) -> Result<()> {
    let sleeper = TokioRecoverySleeper;
    let mut resources = ConformanceResources::new(backend, &sleeper);
    let outcome = exercise_backend_with_resources(backend, &mut resources, profile).await;
    let cleanup = resources.cleanup().await;
    finish(outcome, cleanup)
}

/// Proves that one domain allowlist permits its exact HTTPS destination and
/// prevents an application response from a different HTTPS destination.
///
/// The target image must provide `/bin/sh` and `curl`. The helper tracks and
/// destroys its sandbox even when either fetch probe fails.
pub async fn exercise_network_allowlist(backend: &dyn SandboxBackend, profile: &str) -> Result<()> {
    let sleeper = TokioRecoverySleeper;
    let mut resources = ConformanceResources::new(backend, &sleeper);
    let owner = ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7());
    let outcome =
        crate::conformance_network::exercise(backend, &mut resources, owner, profile).await;
    let cleanup = resources.cleanup().await;
    finish(outcome, cleanup)
}

async fn exercise_backend_with_resources(
    backend: &dyn SandboxBackend,
    resources: &mut ConformanceResources<'_>,
    profile: &str,
) -> Result<()> {
    backend
        .list_managed_sandboxes("backend-conformance".to_owned())
        .await?;
    let workspace_id = Uuid::now_v7();
    let owner = ResourceOwner::agent(workspace_id, Uuid::now_v7());
    let source = resources
        .create_sandbox(sandbox_request(owner, profile, None))
        .await?;
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
    let snapshot = resources.create_snapshot(snapshot_request.clone()).await?;
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
            command: "/bin/sh".to_owned(),
            args: vec!["-c".to_owned(), PROCESS_SCRIPT.to_owned()],
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

    let first = resources
        .create_sandbox(sandbox_request(
            owner,
            profile,
            Some(snapshot.provider_ref.clone()),
        ))
        .await?;
    backend
        .clean_restored_terminals(first.provider_ref.clone())
        .await?;
    let second = resources
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
    resources.track_terminal(&source.provider_ref, &terminal.provider_ref);
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

    crate::conformance_image::exercise(backend, profile, workspace_id).await
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
        consumer: SandboxConsumer::Runtime,
        deployment_id: "backend-conformance".to_owned(),
        profile: profile.to_owned(),
        network: SandboxNetworkPolicy::Open,
        snapshot_provider_ref,
    }
}
