//! Stateful source, snapshot, independent restore, and reconnect smoke flow.

use std::io;

use sandbox_e2b::{E2bAdapterConfig, E2bSandboxBackend};
use sandbox_interface::{
    BackendCreateSnapshotRequest, BackendInputRequest, BackendSnapshotCreateOutcome,
    BackendTerminal, OperationId, ProviderRef, ResourceOwner, SandboxBackend, SnapshotId,
};

use super::support::{
    LiveResources, LiveResult, create_tracked_sandbox, recover_snapshot, sandbox_request,
    terminal_request, wait_for_output,
};

trait LiveStage<T> {
    fn stage(self, stage: &'static str) -> LiveResult<T>;
}

impl<T, E> LiveStage<T> for Result<T, E>
where
    E: std::fmt::Debug,
{
    fn stage(self, stage: &'static str) -> LiveResult<T> {
        self.map_err(|error| io::Error::other(format!("{stage}: {error:?}")).into())
    }
}

pub(super) async fn run(
    backend: &E2bSandboxBackend,
    config: &E2bAdapterConfig,
    owner: ResourceOwner,
    resources: &mut LiveResources,
) -> LiveResult<()> {
    let source = create_tracked_sandbox(backend, sandbox_request(owner, None), resources)
        .await
        .stage("source sandbox creation")?;
    let terminal = backend
        .create_terminal(terminal_request(source.provider_ref.clone()))
        .await
        .stage("source terminal creation")?;
    resources.track_terminal(&source.provider_ref, &terminal);
    write(
        backend,
        source.provider_ref.clone(),
        &terminal,
        b"pwd\ncd /tmp\n",
    )
    .await
    .stage("initial terminal input")?;
    write(
        backend,
        source.provider_ref.clone(),
        &terminal,
        b"printf captured > /tmp/sandbox-state.txt\nnohup sh -c 'sleep 120' >/dev/null 2>&1 & echo $! > /tmp/sandbox-process.pid\ntest \"$PWD\" = /tmp && echo sandbox-live-before\n",
    )
    .await
    .stage("stateful source input")?;
    let before_offset = wait_for_output(
        backend,
        source.provider_ref.clone(),
        &terminal,
        0,
        b"sandbox-live-before",
    )
    .await
    .stage("stateful source output")?;

    let snapshot = snapshot(backend, source.provider_ref.clone(), resources)
        .await
        .stage("snapshot creation")?;
    write(
        backend,
        source.provider_ref.clone(),
        &terminal,
        b"printf changed > /tmp/sandbox-state.txt\ntouch /tmp/sandbox-after.txt\ntest \"$PWD\" = /tmp && echo sandbox-live-after\n",
    )
    .await
    .stage("post-snapshot source input")?;
    wait_for_output(
        backend,
        source.provider_ref.clone(),
        &terminal,
        before_offset,
        b"sandbox-live-after",
    )
    .await
    .stage("post-snapshot source output")?;

    let first = restore(backend, owner, snapshot.clone(), resources)
        .await
        .stage("first snapshot restore")?;
    let second = restore(backend, owner, snapshot, resources)
        .await
        .stage("second snapshot restore")?;
    if first == second {
        return Err(io::Error::other("independent restores reused one sandbox").into());
    }
    let first_terminal = verify_restore(backend, first.clone(), "sandbox-live-restore-one")
        .await
        .stage("first restore verification")?;
    resources.track_terminal(&first, &first_terminal);
    write(
        backend,
        first.clone(),
        &first_terminal,
        b"printf clone-one > /tmp/sandbox-state.txt\n",
    )
    .await
    .stage("first restore mutation")?;
    let second_terminal = verify_restore(backend, second.clone(), "sandbox-live-restore-two")
        .await
        .stage("second restore verification")?;
    resources.track_terminal(&second, &second_terminal);
    reconnect_smoke(config, first, &first_terminal)
        .await
        .stage("terminal reconnect")?;

    let paused = backend
        .pause_sandbox(source.provider_ref.clone())
        .await
        .stage("source pause")?;
    let resumed = backend
        .resume_sandbox(paused.provider_ref)
        .await
        .stage("source resume")?;
    if resumed.provider_ref != source.provider_ref {
        return Err(io::Error::other("pause/resume changed the sandbox identity").into());
    }
    Ok(())
}

async fn snapshot(
    backend: &E2bSandboxBackend,
    source: ProviderRef,
    resources: &mut LiveResources,
) -> LiveResult<ProviderRef> {
    let correlation = format!("sandbox-live-{}", OperationId::new());
    let inventory = backend
        .snapshot_inventory(source.clone(), correlation.clone())
        .await?;
    let request = BackendCreateSnapshotRequest {
        snapshot_id: SnapshotId::new(),
        operation_id: OperationId::new(),
        source_provider_ref: source,
        correlation_name: correlation,
        before: inventory,
    };
    resources.snapshot_request = Some(request.clone());
    let snapshot = match backend.create_snapshot(request.clone()).await? {
        BackendSnapshotCreateOutcome::Created(snapshot) => snapshot.provider_ref,
        BackendSnapshotCreateOutcome::InProgress
        | BackendSnapshotCreateOutcome::DeliveryAmbiguous => {
            recover_snapshot(backend, request).await?
        }
    };
    resources.snapshot = Some(snapshot.clone());
    Ok(snapshot)
}

async fn restore(
    backend: &E2bSandboxBackend,
    owner: ResourceOwner,
    snapshot: ProviderRef,
    resources: &mut LiveResources,
) -> LiveResult<ProviderRef> {
    let sandbox =
        create_tracked_sandbox(backend, sandbox_request(owner, Some(snapshot)), resources).await?;
    backend
        .clean_restored_terminals(sandbox.provider_ref.clone())
        .await?;
    Ok(sandbox.provider_ref)
}

async fn verify_restore(
    backend: &E2bSandboxBackend,
    sandbox: ProviderRef,
    marker: &str,
) -> LiveResult<BackendTerminal> {
    let terminal = backend
        .create_terminal(terminal_request(sandbox.clone()))
        .await?;
    let input = format!(
        "test \"$(cat /tmp/sandbox-state.txt)\" = captured && test ! -e /tmp/sandbox-after.txt && kill -0 \"$(cat /tmp/sandbox-process.pid)\" && echo {marker}\n"
    );
    write(backend, sandbox.clone(), &terminal, input.as_bytes()).await?;
    wait_for_output(backend, sandbox, &terminal, 0, marker.as_bytes()).await?;
    Ok(terminal)
}

async fn reconnect_smoke(
    config: &E2bAdapterConfig,
    sandbox: ProviderRef,
    terminal: &BackendTerminal,
) -> LiveResult<()> {
    let reconnected = E2bSandboxBackend::new(config.clone())?;
    write(
        &reconnected,
        sandbox.clone(),
        terminal,
        b"sleep 1; echo sandbox-live-delayed\n",
    )
    .await?;
    let offset = wait_for_output(
        &reconnected,
        sandbox.clone(),
        terminal,
        0,
        b"sandbox-live-delayed",
    )
    .await?;
    write(
        &reconnected,
        sandbox.clone(),
        terminal,
        b"read answer; echo interactive=$answer\n",
    )
    .await?;
    let second_worker = E2bSandboxBackend::new(config.clone())?;
    write(
        &second_worker,
        sandbox.clone(),
        terminal,
        b"sandbox-answer\n",
    )
    .await?;
    wait_for_output(
        &second_worker,
        sandbox,
        terminal,
        offset,
        b"interactive=sandbox-answer",
    )
    .await?;
    Ok(())
}

async fn write(
    backend: &E2bSandboxBackend,
    sandbox: ProviderRef,
    terminal: &BackendTerminal,
    input: &[u8],
) -> LiveResult<()> {
    backend
        .write_terminal(BackendInputRequest {
            sandbox_provider_ref: sandbox,
            terminal_provider_ref: terminal.provider_ref.clone(),
            input: input.to_vec(),
        })
        .await?;
    Ok(())
}

#[cfg(test)]
#[path = "_tests_/lifecycle_tests.rs"]
mod lifecycle_tests;
