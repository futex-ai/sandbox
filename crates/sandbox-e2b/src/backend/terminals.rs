//! E2B durable PTY creation, ingestion, input, and restored-helper cleanup.

use std::time::Duration;

use sandbox_interface::{
    BackendInputRequest, BackendTerminal, BackendTerminalCreateRequest, Error, ProviderRef,
    ResourceKind, Result, TerminalState,
};

use crate::process::{
    ProcessCommand, ProcessConnection, ProcessOutputCapture, ProcessPtyRequest, ProcessSelector,
};

use super::{
    configured::E2bSandboxBackend,
    mapping,
    terminal_identity::{TerminalIdentity, tagged_terminal, terminal_tag},
};

const TERMINAL_LOG_DIRECTORY: &str = "/tmp/sandbox/terminals";
const RESTORE_CLEANUP: &str = r#"set -eu
if mountpoint -q /drives/me; then
    fusermount3 -u /drives/me || umount -l /drives/me
fi
stop_sandbox_drive_helpers() {
    pkill -TERM -f -- '[s]andbox-drive-' || true
    sandbox_drive_stop_attempt=0
    while pgrep -f -- '[s]andbox-drive-' >/dev/null && [ "$sandbox_drive_stop_attempt" -lt 20 ]; do
        sleep 0.1
        sandbox_drive_stop_attempt=$((sandbox_drive_stop_attempt + 1))
    done
    if pgrep -f -- '[s]andbox-drive-' >/dev/null; then
        pkill -KILL -f -- '[s]andbox-drive-' || true
        sandbox_drive_stop_attempt=0
        while pgrep -f -- '[s]andbox-drive-' >/dev/null && [ "$sandbox_drive_stop_attempt" -lt 20 ]; do
            sleep 0.1
            sandbox_drive_stop_attempt=$((sandbox_drive_stop_attempt + 1))
        done
    fi
    ! pgrep -f -- '[s]andbox-drive-' >/dev/null
}
stop_sandbox_drive_helpers
rm -rf /tmp/sandbox-drive /tmp/sandbox/terminals
"#;

pub(super) async fn clean_restored(
    backend: &E2bSandboxBackend,
    sandbox_ref: ProviderRef,
) -> Result<()> {
    let connection = mapping::connection(backend, &sandbox_ref).await?;
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
        .run(
            connection,
            ProcessCommand {
                command: "/bin/sh".to_owned(),
                args: vec!["-lc".to_owned(), RESTORE_CLEANUP.to_owned()],
                cwd: None,
                output_capture: ProcessOutputCapture::HardLimit { max_bytes: 4096 },
                timeout: Duration::from_secs(300),
                read_only: false,
            },
        )
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
    let connection = mapping::connection(backend, &request.sandbox_provider_ref).await?;
    if let Some(terminal) = recover_connected(backend, &connection, request.terminal_id).await? {
        return Ok(terminal);
    }
    let tag = terminal_tag(
        backend.config.runtime_conventions().terminal_tag_prefix(),
        request.terminal_id,
    );
    let directory = backend
        .processes
        .run(
            connection.clone(),
            ProcessCommand {
                command: "/bin/mkdir".to_owned(),
                args: vec!["-p".to_owned(), TERMINAL_LOG_DIRECTORY.to_owned()],
                cwd: None,
                output_capture: ProcessOutputCapture::HardLimit { max_bytes: 4096 },
                timeout: Duration::from_secs(300),
                read_only: false,
            },
        )
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
    let connection = mapping::connection(backend, &request.sandbox_provider_ref).await?;
    recover_connected(backend, &connection, request.terminal_id).await
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
    let connection = mapping::connection(backend, &sandbox_ref).await?;
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
    let connection = mapping::connection(backend, &request.sandbox_provider_ref).await?;
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
    let connection = mapping::connection(backend, &sandbox_ref).await?;
    let tag = terminal_tag(
        backend.config.runtime_conventions().terminal_tag_prefix(),
        identity.terminal_id(),
    );
    backend
        .processes
        .kill(connection, ProcessSelector::Tag(tag))
        .await
}
