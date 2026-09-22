//! E2B terminal input, close, and restored-state cleanup operations.

use sandbox_interface::{BackendInputRequest, Error, ProviderRef, Result, TerminalState};

use crate::process::ProcessSelector;

use super::{
    configured::E2bSandboxBackend,
    mapping,
    terminal_identity::{TerminalIdentity, tagged_terminal, terminal_tag},
    terminal_storage::restore_cleanup_command,
};

const TRUSTED_PROCESS_USER: &str = "root";

pub(super) async fn clean_restored(
    backend: &E2bSandboxBackend,
    sandbox_ref: ProviderRef,
) -> Result<()> {
    let connection = mapping::connection(backend, &sandbox_ref)
        .await?
        .with_user(TRUSTED_PROCESS_USER);
    let processes = backend.processes.list(connection.clone()).await?;
    let terminal_tag_prefix = backend.config.runtime_conventions().terminal_tag_prefix();
    for process in processes {
        if tagged_terminal(&process, terminal_tag_prefix).is_none() {
            continue;
        }
        let Some(tag) = process.tag else {
            continue;
        };
        backend
            .processes
            .kill(connection.clone(), ProcessSelector::Tag(tag))
            .await?;
    }
    let cleanup = backend
        .processes
        .run(connection, restore_cleanup_command())
        .await?;
    if !cleanup.succeeded() {
        return Err(Error::internal_message(
            "restored E2B terminal helper cleanup failed",
        ));
    }
    Ok(())
}

pub(super) async fn write(backend: &E2bSandboxBackend, request: BackendInputRequest) -> Result<()> {
    let identity = TerminalIdentity::parse(&request.terminal_provider_ref)?;
    let connection = mapping::connection(backend, &request.sandbox_provider_ref)
        .await?
        .with_user(TRUSTED_PROCESS_USER);
    if identity
        .resolve(
            &backend.processes.list(connection.clone()).await?,
            backend.config.runtime_conventions().terminal_tag_prefix(),
        )?
        .is_none()
    {
        return Err(Error::TerminalStateConflict {
            state: TerminalState::Exited,
        });
    }
    let tag = terminal_tag(
        backend.config.runtime_conventions().terminal_tag_prefix(),
        identity.terminal_id(),
    );
    backend
        .processes
        .send_input(connection, ProcessSelector::Tag(tag), request.input)
        .await
}

pub(super) async fn close(
    backend: &E2bSandboxBackend,
    sandbox_ref: ProviderRef,
    terminal_ref: ProviderRef,
) -> Result<()> {
    let identity = TerminalIdentity::parse(&terminal_ref)?;
    let connection = mapping::connection(backend, &sandbox_ref)
        .await?
        .with_user(TRUSTED_PROCESS_USER);
    let tag = terminal_tag(
        backend.config.runtime_conventions().terminal_tag_prefix(),
        identity.terminal_id(),
    );
    backend
        .processes
        .kill(connection, ProcessSelector::Tag(tag))
        .await
}
