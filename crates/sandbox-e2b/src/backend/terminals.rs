//! E2B durable PTY creation, ingestion, input, and restored-helper cleanup.

use sandbox_interface::{
    BackendInputRequest, BackendTerminal, BackendTerminalCreateRequest, Error,
    FILE_TRANSFER_MAX_BYTES, ProviderRef, ResourceKind, Result, TerminalState,
};

use crate::process::{ProcessConnection, ProcessPtyRequest, ProcessSelector};

use super::{
    configured::E2bSandboxBackend,
    mapping,
    terminal_identity::{TerminalIdentity, tagged_terminal, terminal_tag},
    terminal_storage::{TERMINAL_LOG_DIRECTORY, create_directory_command, restore_cleanup_command},
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

pub(super) async fn create(
    backend: &E2bSandboxBackend,
    request: BackendTerminalCreateRequest,
) -> Result<BackendTerminal> {
    validate_transcript_limit(request.provider_log_limit)?;
    let connection = mapping::connection(backend, &request.sandbox_provider_ref)
        .await?
        .with_user(TRUSTED_PROCESS_USER);
    if let Some(terminal) = recover_connected(backend, &connection, request.terminal_id).await? {
        return Ok(terminal);
    }
    let tag = terminal_tag(
        backend.config.runtime_conventions().terminal_tag_prefix(),
        request.terminal_id,
    );
    let directory = backend
        .processes
        .run(connection.clone(), create_directory_command())
        .await?;
    if !directory.succeeded() {
        return Err(Error::internal_message(
            "E2B terminal log directory creation failed",
        ));
    }
    let log_path = format!("{TERMINAL_LOG_DIRECTORY}/{}.log", request.terminal_id);
    let process = backend
        .processes
        .start_pty(
            connection,
            ProcessPtyRequest {
                tag,
                log_path: log_path.clone(),
                log_limit: request.provider_log_limit,
                workload_user: backend
                    .config
                    .runtime_conventions()
                    .workload_user()
                    .to_owned(),
                cwd: request.cwd,
            },
        )
        .await?;
    Ok(BackendTerminal {
        provider_ref: TerminalIdentity::new(process.pid, request.terminal_id).provider_ref(),
        provider_log_path: log_path,
        state: TerminalState::Ready,
    })
}

pub(super) async fn recover(
    backend: &E2bSandboxBackend,
    request: BackendTerminalCreateRequest,
) -> Result<Option<BackendTerminal>> {
    validate_transcript_limit(request.provider_log_limit)?;
    let connection = mapping::connection(backend, &request.sandbox_provider_ref)
        .await?
        .with_user(TRUSTED_PROCESS_USER);
    recover_connected(backend, &connection, request.terminal_id).await
}

fn validate_transcript_limit(provider_log_limit: usize) -> Result<()> {
    if provider_log_limit > FILE_TRANSFER_MAX_BYTES {
        return Err(Error::InvalidLength {
            field: "provider_log_limit",
            minimum: 0,
            maximum: FILE_TRANSFER_MAX_BYTES,
        });
    }
    Ok(())
}

async fn recover_connected(
    backend: &E2bSandboxBackend,
    connection: &ProcessConnection,
    terminal_id: sandbox_interface::TerminalId,
) -> Result<Option<BackendTerminal>> {
    let tag = terminal_tag(
        backend.config.runtime_conventions().terminal_tag_prefix(),
        terminal_id,
    );
    let matching = backend
        .processes
        .list(connection.clone())
        .await?
        .into_iter()
        .filter(|process| process.tag.as_deref() == Some(tag.as_str()))
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [] => Ok(None),
        [process] => Ok(Some(BackendTerminal {
            provider_ref: TerminalIdentity::new(process.pid, terminal_id).provider_ref(),
            provider_log_path: format!("{TERMINAL_LOG_DIRECTORY}/{terminal_id}.log"),
            state: TerminalState::Ready,
        })),
        _ => Err(Error::internal_message(
            "multiple E2B terminal processes matched one consumer handle",
        )),
    }
}

pub(super) async fn inspect(
    backend: &E2bSandboxBackend,
    sandbox_ref: ProviderRef,
    terminal_ref: ProviderRef,
) -> Result<BackendTerminal> {
    let identity = TerminalIdentity::parse(&terminal_ref)?;
    let connection = mapping::connection(backend, &sandbox_ref)
        .await?
        .with_user(TRUSTED_PROCESS_USER);
    if identity
        .resolve(
            backend.processes.list(connection).await?,
            backend.config.runtime_conventions().terminal_tag_prefix(),
        )?
        .is_none()
    {
        return Err(Error::NotFound {
            resource: ResourceKind::Terminal,
        });
    }
    Ok(BackendTerminal {
        provider_ref: terminal_ref,
        provider_log_path: format!("{TERMINAL_LOG_DIRECTORY}/{}.log", identity.terminal_id()),
        state: TerminalState::Ready,
    })
}

pub(super) async fn write(backend: &E2bSandboxBackend, request: BackendInputRequest) -> Result<()> {
    let identity = TerminalIdentity::parse(&request.terminal_provider_ref)?;
    let connection = mapping::connection(backend, &request.sandbox_provider_ref)
        .await?
        .with_user(TRUSTED_PROCESS_USER);
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
