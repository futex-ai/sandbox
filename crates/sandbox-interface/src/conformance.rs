//! Reusable backend conformance harness for provider implementations.

use std::time::Duration;

use uuid::Uuid;

use crate::{
    BackendCreateSandboxRequest, BackendCreateSnapshotRequest, BackendInputRequest,
    BackendInspectSnapshotRequest, BackendOutputRequest, BackendPortIngressRequest,
    BackendTerminalCreateRequest, Error, OperationId, ProviderRef, ResourceOwner, Result,
    SandboxConsumer, SandboxFiles, SandboxId, SandboxImagePreparation, SandboxLifecycle,
    SandboxLifetime, SandboxNetworkPolicy, SandboxPortIngress, SandboxProcesses,
    SandboxResumableLifecycle, SandboxSnapshots, SandboxTerminals, SnapshotId, TerminalId,
    conformance_image, conformance_network,
    conformance_resources::{ConformanceResources, TokioRecoverySleeper, finish},
};

pub use crate::conformance_files::exercise_files;
pub use crate::conformance_probes::{exercise_lifecycle, exercise_snapshots};
pub use crate::conformance_process::exercise_processes;

/// Explicit capabilities used by the complete provider conformance flow.
///
/// Providers with fewer capabilities call the independent probes instead.
#[derive(Clone, Copy)]
pub struct BackendConformance<'a> {
    /// Core lifecycle operations.
    pub lifecycle: &'a dyn SandboxLifecycle,
    /// Optional provider support for pause and resume.
    pub resumable: &'a dyn SandboxResumableLifecycle,
    /// Saved snapshot operations.
    pub snapshots: &'a dyn SandboxSnapshots,
    /// Caller-owned source preparation.
    pub images: &'a dyn SandboxImagePreparation,
    /// Collected, streaming, and stateless process execution.
    pub processes: &'a dyn SandboxProcesses,
    /// Regular-file transfer.
    pub files: &'a dyn SandboxFiles,
    /// Persistent terminal operations and restored cleanup.
    pub terminals: &'a dyn SandboxTerminals,
    /// Exact private port routing.
    pub ingress: &'a dyn SandboxPortIngress,
}

/// Exercises all guarantees of a provider with these supported capabilities.
///
/// The target image must provide `/bin/sh`; the split-output probe supplies its
/// own shell script.
pub async fn exercise_backend(backend: BackendConformance<'_>, profile: &str) -> Result<()> {
    exercise_one_shot_lifetime(backend.lifecycle, profile).await?;
    let sleeper = TokioRecoverySleeper;
    let mut resources = ConformanceResources::new(&sleeper);
    let outcome = exercise_backend_with_resources(backend, &mut resources, profile).await;
    let cleanup = resources.cleanup().await;
    finish(outcome, cleanup)
}

/// Proves that one domain allowlist permits its exact HTTPS destination and
/// prevents an application response from a different HTTPS destination.
///
/// The target image must provide `/bin/sh` and `curl`. The helper tracks and
/// destroys its sandbox even when either fetch probe fails. Both curl commands
/// disable startup configuration before processing any other option.
pub async fn exercise_network_allowlist(
    lifecycle: &dyn SandboxLifecycle,
    processes: &dyn SandboxProcesses,
    profile: &str,
) -> Result<()> {
    let sleeper = TokioRecoverySleeper;
    let mut resources = ConformanceResources::new(&sleeper);
    let owner = ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7());
    let outcome =
        conformance_network::exercise(lifecycle, processes, &mut resources, owner, profile).await;
    let cleanup = resources.cleanup().await;
    finish(outcome, cleanup)
}

/// Proves that a backend can create, recover, and explicitly destroy a bounded
/// one-shot sandbox without relying on idle pause or resume behavior.
pub async fn exercise_one_shot_lifetime(
    backend: &dyn SandboxLifecycle,
    profile: &str,
) -> Result<()> {
    let sleeper = TokioRecoverySleeper;
    let mut resources = ConformanceResources::new(&sleeper);
    let owner = ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7());
    let request = sandbox_request(
        owner,
        profile,
        None,
        SandboxLifetime::OneShot {
            max_lifetime: Duration::from_secs(60),
        },
    );
    let outcome = match resources.create_sandbox(backend, request).await {
        Ok(sandbox) => backend.destroy_sandbox(sandbox.provider_ref).await,
        Err(error) => Err(error),
    };
    let cleanup = resources.cleanup().await;
    finish(outcome, cleanup)
}

async fn exercise_backend_with_resources<'a>(
    backend: BackendConformance<'a>,
    resources: &mut ConformanceResources<'a>,
    profile: &str,
) -> Result<()> {
    backend
        .lifecycle
        .list_managed_sandboxes("backend-conformance".to_owned())
        .await?;
    let workspace_id = Uuid::now_v7();
    let owner = ResourceOwner::agent(workspace_id, Uuid::now_v7());
    let source = resources
        .create_sandbox(
            backend.lifecycle,
            sandbox_request(owner, profile, None, SandboxLifetime::IdleAutoPause),
        )
        .await?;
    backend
        .lifecycle
        .inspect_sandbox(source.provider_ref.clone())
        .await?;
    let paused = backend
        .resumable
        .pause_sandbox(source.provider_ref.clone())
        .await?;
    backend
        .resumable
        .resume_sandbox(paused.provider_ref)
        .await?;
    let ingress = backend
        .ingress
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
        .snapshots
        .snapshot_inventory(source.provider_ref.clone(), correlation.clone())
        .await?;
    let snapshot_request = BackendCreateSnapshotRequest {
        snapshot_id: SnapshotId::new(),
        operation_id: OperationId::new(),
        source_provider_ref: source.provider_ref.clone(),
        correlation_name: correlation,
        before,
    };
    let snapshot = resources
        .create_snapshot(backend.snapshots, snapshot_request.clone())
        .await?;
    backend
        .snapshots
        .inspect_snapshot(BackendInspectSnapshotRequest {
            provider_ref: snapshot.provider_ref.clone(),
            source_provider_ref: source.provider_ref.clone(),
            correlation_name: snapshot_request.correlation_name,
        })
        .await?;
    exercise_files(backend.files, source.provider_ref.clone()).await?;
    exercise_processes(backend.processes, source.provider_ref.clone()).await?;

    let first = resources
        .create_sandbox(
            backend.lifecycle,
            sandbox_request(
                owner,
                profile,
                Some(snapshot.provider_ref.clone()),
                SandboxLifetime::IdleAutoPause,
            ),
        )
        .await?;
    backend
        .terminals
        .clean_restored_terminals(first.provider_ref.clone())
        .await?;
    let second = resources
        .create_sandbox(
            backend.lifecycle,
            sandbox_request(
                owner,
                profile,
                Some(snapshot.provider_ref.clone()),
                SandboxLifetime::IdleAutoPause,
            ),
        )
        .await?;
    backend
        .terminals
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
    let terminal = backend
        .terminals
        .create_terminal(terminal_request.clone())
        .await?;
    resources.track_terminal(
        backend.terminals,
        &source.provider_ref,
        &terminal.provider_ref,
    );
    let recovered_terminal = backend
        .terminals
        .recover_terminal_create(terminal_request)
        .await?
        .ok_or_else(|| Error::internal_message("backend did not recover a created terminal"))?;
    if recovered_terminal.provider_ref != terminal.provider_ref {
        return Err(Error::internal_message(
            "backend terminal recovery changed provider identity",
        ));
    }
    backend
        .terminals
        .inspect_terminal(source.provider_ref.clone(), terminal.provider_ref.clone())
        .await?;
    backend
        .terminals
        .write_terminal(BackendInputRequest {
            sandbox_provider_ref: source.provider_ref.clone(),
            terminal_provider_ref: terminal.provider_ref.clone(),
            input: b"printf 'conformance\\n'\n".to_vec(),
        })
        .await?;
    backend
        .terminals
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

    conformance_image::exercise(
        backend.lifecycle,
        backend.snapshots,
        backend.images,
        profile,
        workspace_id,
    )
    .await
}

pub(crate) fn sandbox_request(
    owner: ResourceOwner,
    profile: &str,
    snapshot_provider_ref: Option<ProviderRef>,
    lifetime: SandboxLifetime,
) -> BackendCreateSandboxRequest {
    BackendCreateSandboxRequest {
        size: None,
        sandbox_id: SandboxId::new(),
        operation_id: OperationId::new(),
        owner,
        consumer: SandboxConsumer::Runtime,
        lifetime,
        deployment_id: "backend-conformance".to_owned(),
        profile: profile.to_owned(),
        network: SandboxNetworkPolicy::Open,
        snapshot_provider_ref,
    }
}

#[cfg(test)]
#[path = "_tests_/conformance_partial_tests.rs"]
mod conformance_partial_tests;
