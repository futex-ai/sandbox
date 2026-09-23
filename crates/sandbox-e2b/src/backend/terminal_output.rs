//! E2B durable PTY output reads and bounded long-polling.

use std::time::Duration;

use sandbox_interface::{
    BackendOutputRequest, BackendTerminalOutput, Error, ResourceKind, Result,
    TERMINAL_OUTPUT_MAX_WAIT, TerminalState,
};

use crate::process::ProcessRegularFileRequest;

use super::{
    configured::E2bSandboxBackend, mapping, terminal_identity::TerminalIdentity, terminal_record,
    terminal_storage::TERMINAL_LOG_DIRECTORY,
};

const PROVIDER_READ_ALLOWANCE: Duration = Duration::from_secs(5);
const HELPER_RETURN_RESERVE: Duration = Duration::from_millis(100);
const TRUSTED_PROCESS_USER: &str = "root";

pub(super) async fn read(
    backend: &E2bSandboxBackend,
    request: BackendOutputRequest,
) -> Result<BackendTerminalOutput> {
    if request.wait > TERMINAL_OUTPUT_MAX_WAIT {
        return Err(Error::InvalidSeconds {
            field: "wait",
            minimum: 0,
            maximum: TERMINAL_OUTPUT_MAX_WAIT.as_secs(),
        });
    }
    let provider_log_limit = match u64::try_from(request.provider_log_limit) {
        Ok(provider_log_limit) => provider_log_limit,
        Err(source) => {
            return Err(Error::internal_with(
                source,
                "convert E2B provider log limit",
            ));
        }
    };
    let identity = TerminalIdentity::parse(&request.terminal_provider_ref)?;
    let log_name = format!("{}.log", identity.terminal_id());
    let expected_log_path = format!("{TERMINAL_LOG_DIRECTORY}/{log_name}");
    if request.provider_log_path != expected_log_path {
        return Err(Error::InvalidFilePath {
            field: "provider_log_path",
        });
    }
    let connection = mapping::connection(backend, &request.sandbox_provider_ref)
        .await?
        .with_user(TRUSTED_PROCESS_USER);
    let started = tokio::time::Instant::now();
    let provider_deadline = request
        .wait
        .checked_add(PROVIDER_READ_ALLOWANCE)
        .and_then(|wait| started.checked_add(wait))
        .ok_or(Error::InvalidSeconds {
            field: "wait",
            minimum: 0,
            maximum: TERMINAL_OUTPUT_MAX_WAIT.as_secs(),
        })?;
    let mut retry_post_read_growth = true;
    let (chunk, state) = loop {
        let listed = match tokio::time::timeout_at(
            provider_deadline,
            backend.processes.list(connection.clone()),
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => return Err(provider_read_timeout(backend)),
        };
        let state = match tokio::time::timeout_at(
            provider_deadline,
            terminal_record::resolve_state(
                backend,
                &connection,
                identity,
                &listed,
                backend.config.runtime_conventions().terminal_tag_prefix(),
                terminal_record::IDENTITY_READ_TIMEOUT,
                Some(identity_completion_deadline(provider_deadline)),
            ),
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => return Err(provider_read_timeout(backend)),
        };
        let helper_timeout =
            provider_deadline.saturating_duration_since(tokio::time::Instant::now());
        let chunk = match tokio::time::timeout_at(
            provider_deadline,
            backend.processes.read_regular_file(
                connection.clone(),
                ProcessRegularFileRequest {
                    root: TERMINAL_LOG_DIRECTORY.to_owned(),
                    path: log_name.clone(),
                    offset: request.offset,
                    max_bytes: request.max_bytes,
                    timeout: helper_timeout,
                    completion_deadline: None,
                },
            ),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => return Err(provider_read_timeout(backend)),
        };
        let chunk = match chunk {
            Ok(chunk) => Some(chunk),
            Err(Error::NotFound { .. }) => None,
            Err(error) => return Err(error),
        };
        let Some(chunk) = chunk else {
            if state == TerminalState::Exited {
                return Err(Error::NotFound {
                    resource: ResourceKind::Terminal,
                });
            }
            if started.elapsed() >= request.wait {
                return Ok(empty_output(&request));
            }
            wait_for_output(&request, started).await;
            continue;
        };
        if retry_post_read_growth && chunk.bytes.is_empty() && chunk.total_size > request.offset {
            retry_post_read_growth = false;
            continue;
        }
        if !chunk.bytes.is_empty()
            || state == TerminalState::Exited
            || started.elapsed() >= request.wait
        {
            break (chunk, state);
        }
        wait_for_output(&request, started).await;
    };
    let byte_count = match u64::try_from(chunk.bytes.len()) {
        Ok(byte_count) => byte_count,
        Err(source) => {
            return Err(Error::internal_with(
                source,
                "convert E2B terminal output byte count",
            ));
        }
    };
    let next_offset = request
        .offset
        .checked_add(byte_count)
        .ok_or_else(|| Error::internal_message("E2B terminal output offset overflowed"))?;
    Ok(BackendTerminalOutput {
        bytes: chunk.bytes,
        next_offset,
        total_size: chunk.total_size,
        state,
        exit_code: None,
        overflowed: chunk.total_size >= provider_log_limit,
    })
}

fn empty_output(request: &BackendOutputRequest) -> BackendTerminalOutput {
    BackendTerminalOutput {
        bytes: Vec::new(),
        next_offset: request.offset,
        total_size: request.offset,
        state: TerminalState::Ready,
        exit_code: None,
        overflowed: false,
    }
}

async fn wait_for_output(request: &BackendOutputRequest, started: tokio::time::Instant) {
    let remaining = request.wait.saturating_sub(started.elapsed());
    tokio::time::sleep(remaining.min(Duration::from_millis(250))).await;
}

fn provider_read_timeout(backend: &E2bSandboxBackend) -> Error {
    Error::BackendUnavailable {
        backend_id: backend.config.backend_id().to_owned(),
    }
}

fn identity_completion_deadline(provider_deadline: tokio::time::Instant) -> tokio::time::Instant {
    provider_deadline
        .checked_sub(HELPER_RETURN_RESERVE)
        .unwrap_or(provider_deadline)
}
