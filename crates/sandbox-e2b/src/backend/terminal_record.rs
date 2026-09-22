//! Trusted, versioned terminal identity records stored beside transcripts.

use std::{num::NonZeroU32, time::Duration};

use sandbox_interface::{Error, OperationId, ResourceKind, Result, TerminalId, TerminalState};
use serde::Deserialize;

use crate::process::{ProcessConnection, ProcessInfo, ProcessRegularFileRequest};

use super::{
    configured::E2bSandboxBackend,
    terminal_identity::{TerminalIdentity, terminal_tag},
    terminal_storage::{TERMINAL_LOG_DIRECTORY, identity_file_name},
};

const IDENTITY_RECORD_MAX_BYTES: usize = 1024;
pub(super) const IDENTITY_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TerminalRecord {
    identity: TerminalIdentity,
    operation_id: OperationId,
    tag: String,
}

impl TerminalRecord {
    pub(super) fn ensure_request(
        &self,
        terminal_id: TerminalId,
        operation_id: OperationId,
    ) -> Result<()> {
        if self.identity.terminal_id() != terminal_id || self.operation_id != operation_id {
            return Err(Error::internal_message(
                "E2B terminal identity record does not match its create request",
            ));
        }
        Ok(())
    }

    pub(super) fn ensure_identity(&self, identity: TerminalIdentity) -> Result<()> {
        if self.identity != identity {
            return Err(Error::NotFound {
                resource: ResourceKind::Terminal,
            });
        }
        Ok(())
    }

    pub(super) fn ensure_tag(&self, tag: &str) -> Result<()> {
        if self.tag != tag {
            return Err(Error::internal_message(
                "E2B terminal identity record has an unexpected process tag",
            ));
        }
        Ok(())
    }

    pub(super) const fn identity(&self) -> TerminalIdentity {
        self.identity
    }

    pub(super) fn state(&self, processes: &[ProcessInfo]) -> Result<TerminalState> {
        let matching = processes
            .iter()
            .filter(|process| process.tag.as_deref() == Some(self.tag.as_str()))
            .collect::<Vec<_>>();
        match matching.as_slice() {
            [] => Ok(TerminalState::Exited),
            [process] if process.pid == self.identity.pid() => Ok(TerminalState::Ready),
            [_] => Err(Error::internal_message(
                "E2B terminal identity record conflicts with its tagged process",
            )),
            _ => Err(Error::internal_message(
                "multiple E2B terminal processes matched one identity record",
            )),
        }
    }
}

pub(super) async fn resolve_state(
    backend: &E2bSandboxBackend,
    connection: &ProcessConnection,
    identity: TerminalIdentity,
    processes: &[ProcessInfo],
    terminal_tag_prefix: &str,
    read_timeout: Duration,
    completion_deadline: Option<tokio::time::Instant>,
) -> Result<TerminalState> {
    if let Some(record) = read(
        backend,
        connection,
        identity.terminal_id(),
        read_timeout,
        completion_deadline,
    )
    .await?
    {
        record.ensure_identity(identity)?;
        record.ensure_tag(&terminal_tag(terminal_tag_prefix, identity.terminal_id()))?;
        return record.state(processes);
    }
    match identity.resolve(processes, terminal_tag_prefix) {
        Ok(Some(_)) => Ok(TerminalState::Ready),
        Ok(None)
        | Err(Error::NotFound {
            resource: ResourceKind::Terminal,
        }) => Err(Error::NotFound {
            resource: ResourceKind::Terminal,
        }),
        Err(error) => Err(error),
    }
}

pub(super) async fn read(
    backend: &E2bSandboxBackend,
    connection: &ProcessConnection,
    terminal_id: TerminalId,
    timeout: Duration,
    completion_deadline: Option<tokio::time::Instant>,
) -> Result<Option<TerminalRecord>> {
    let chunk = backend
        .processes
        .read_regular_file(
            connection.clone(),
            ProcessRegularFileRequest {
                root: TERMINAL_LOG_DIRECTORY.to_owned(),
                path: identity_file_name(terminal_id),
                offset: 0,
                max_bytes: IDENTITY_RECORD_MAX_BYTES,
                timeout,
                completion_deadline,
            },
        )
        .await;
    let chunk = match chunk {
        Ok(chunk) => chunk,
        Err(Error::NotFound {
            resource: ResourceKind::File,
        }) => return Ok(None),
        Err(error) => return Err(error),
    };
    let byte_count = match u64::try_from(chunk.bytes.len()) {
        Ok(byte_count) => byte_count,
        Err(source) => {
            return Err(Error::internal_with(
                source,
                "convert E2B terminal identity record size",
            ));
        }
    };
    if chunk.total_size != byte_count || chunk.bytes.len() > IDENTITY_RECORD_MAX_BYTES {
        return Err(Error::internal_message(
            "E2B terminal identity record was not read completely",
        ));
    }
    decode(&chunk.bytes).map(Some)
}

fn decode(bytes: &[u8]) -> Result<TerminalRecord> {
    let wire: TerminalRecordWire = match serde_json::from_slice(bytes) {
        Ok(wire) => wire,
        Err(source) => {
            return Err(Error::internal_with(
                source,
                "decode E2B terminal identity record",
            ));
        }
    };
    Ok(TerminalRecord {
        identity: TerminalIdentity::new(wire.pid.get(), wire.terminal_id),
        operation_id: wire.operation_id,
        tag: wire.tag,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TerminalRecordWire {
    #[serde(rename = "schema")]
    _schema: TerminalRecordSchema,
    pid: NonZeroU32,
    terminal_id: TerminalId,
    operation_id: OperationId,
    tag: String,
}

#[derive(Deserialize)]
enum TerminalRecordSchema {
    #[serde(rename = "sandbox-e2b-terminal-identity-v1")]
    V1,
}

#[cfg(test)]
#[path = "_tests_/terminal_record_tests.rs"]
mod terminal_record_tests;
