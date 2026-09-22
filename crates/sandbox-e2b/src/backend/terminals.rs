//! E2B durable PTY creation, recovery, and inspection.

use sandbox_interface::{
    BackendTerminal, BackendTerminalCreateRequest, Error, FILE_TRANSFER_MAX_BYTES, ProviderRef,
    ResourceKind, Result, TerminalState,
};

use crate::process::{ProcessConnection, ProcessPtyRequest};

use super::{
    configured::E2bSandboxBackend,
    mapping,
    terminal_identity::{TerminalIdentity, terminal_tag},
    terminal_record,
    terminal_storage::{TERMINAL_LOG_DIRECTORY, create_directory_command, identity_path},
};

const TRUSTED_PROCESS_USER: &str = "root";

pub(super) async fn create(
    backend: &E2bSandboxBackend,
    request: BackendTerminalCreateRequest,
) -> Result<BackendTerminal> {
    validate_transcript_limit(request.provider_log_limit)?;
    let connection = mapping::connection(backend, &request.sandbox_provider_ref)
        .await?
        .with_user(TRUSTED_PROCESS_USER);
    if let Some(terminal) = recover_connected(
        backend,
        &connection,
        request.terminal_id,
        request.operation_id,
    )
    .await?
    {
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
                identity_path: identity_path(request.terminal_id),
                log_limit: request.provider_log_limit,
                workload_user: backend
                    .config
                    .runtime_conventions()
                    .workload_user()
                    .to_owned(),
                terminal_id: request.terminal_id,
                operation_id: request.operation_id,
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
    recover_connected(
        backend,
        &connection,
        request.terminal_id,
        request.operation_id,
    )
    .await
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
    operation_id: sandbox_interface::OperationId,
) -> Result<Option<BackendTerminal>> {
    let tag = terminal_tag(
        backend.config.runtime_conventions().terminal_tag_prefix(),
        terminal_id,
    );
    let processes = backend.processes.list(connection.clone()).await?;
    let matching = processes
        .iter()
        .filter(|process| process.tag.as_deref() == Some(tag.as_str()))
        .collect::<Vec<_>>();
    if matching.len() > 1 {
        return Err(Error::internal_message(
            "multiple E2B terminal processes matched one consumer handle",
        ));
    }
    let record = terminal_record::read(backend, connection, terminal_id).await?;
    if let Some(record) = record {
        record.ensure_request(terminal_id, operation_id)?;
        record.ensure_tag(&tag)?;
        return Ok(Some(BackendTerminal {
            provider_ref: record.identity().provider_ref(),
            provider_log_path: format!("{TERMINAL_LOG_DIRECTORY}/{terminal_id}.log"),
            state: record.state(&processes)?,
        }));
    }
    Ok(matching.first().map(|process| BackendTerminal {
        provider_ref: TerminalIdentity::new(process.pid, terminal_id).provider_ref(),
        provider_log_path: format!("{TERMINAL_LOG_DIRECTORY}/{terminal_id}.log"),
        state: TerminalState::Ready,
    }))
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
    let processes = backend.processes.list(connection.clone()).await?;
    let live = identity.resolve(
        &processes,
        backend.config.runtime_conventions().terminal_tag_prefix(),
    );
    let state = match live {
        Ok(Some(_)) => {
            match terminal_record::read(backend, &connection, identity.terminal_id()).await? {
                Some(record) => {
                    record.ensure_identity(identity)?;
                    record.ensure_tag(&terminal_tag(
                        backend.config.runtime_conventions().terminal_tag_prefix(),
                        identity.terminal_id(),
                    ))?;
                    record.state(&processes)?
                }
                None => TerminalState::Ready,
            }
        }
        Ok(None) | Err(Error::NotFound { .. }) => {
            let record = terminal_record::read(backend, &connection, identity.terminal_id())
                .await?
                .ok_or(Error::NotFound {
                    resource: ResourceKind::Terminal,
                })?;
            record.ensure_identity(identity)?;
            record.ensure_tag(&terminal_tag(
                backend.config.runtime_conventions().terminal_tag_prefix(),
                identity.terminal_id(),
            ))?;
            record.state(&processes)?
        }
        Err(error) => return Err(error),
    };
    Ok(BackendTerminal {
        provider_ref: terminal_ref,
        provider_log_path: format!("{TERMINAL_LOG_DIRECTORY}/{}.log", identity.terminal_id()),
        state,
    })
}
