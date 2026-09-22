//! Relative and absolute deadlines for bounded provider helper processes.

use std::time::Duration;

use sandbox_interface::{Error as DomainError, Result as DomainResult};

use crate::error::Error;

use super::{
    connect::ConnectProcessTransport,
    connect_helpers::{CollectionDeadlines, CollectionMode},
    mapping::map_result,
    types::{ProcessCommand, ProcessConnection, ProcessRunOutput},
    wire::command_start,
};

pub(super) const KILL_DEADLINE: Duration = Duration::from_secs(3);

impl ConnectProcessTransport {
    pub(super) async fn run_for(
        &self,
        connection: ProcessConnection,
        command: ProcessCommand,
        timeout: Duration,
    ) -> DomainResult<ProcessRunOutput> {
        let execution_deadline = tokio::time::Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| unavailable(&self.backend_id))?;
        self.run_before(connection, command, execution_deadline, None)
            .await
    }

    async fn run_before(
        &self,
        connection: ProcessConnection,
        command: ProcessCommand,
        execution_deadline: tokio::time::Instant,
        cleanup_deadline: Option<tokio::time::Instant>,
    ) -> DomainResult<ProcessRunOutput> {
        let output_capture = command.output_capture;
        let read_only = command.read_only;
        let body = command_start(command);
        let collected = self
            .collect_before(
                connection,
                "Start",
                &body,
                CollectionDeadlines {
                    execution: execution_deadline,
                    cleanup: cleanup_deadline,
                },
                output_capture,
                CollectionMode::RunOneShot,
            )
            .await;
        let events = match collected {
            Err(Error::ResponseTooLarge) if read_only => {
                return Err(DomainError::ReadOnlyOutputTooLarge);
            }
            result => map_result(result, false, &self.backend_id)?,
        };
        Ok(ProcessRunOutput {
            bytes: events.bytes,
            exit_code: events.exit_code,
            exited: events.exited,
            output_truncated: events.output_truncated,
        })
    }

    pub(super) async fn run_helper(
        &self,
        connection: ProcessConnection,
        command: ProcessCommand,
        timeout: Duration,
    ) -> DomainResult<ProcessRunOutput> {
        helper_output(
            self.run_for(connection, command, timeout).await,
            &self.backend_id,
        )
    }

    pub(super) async fn run_helper_before(
        &self,
        connection: ProcessConnection,
        command: ProcessCommand,
        completion_deadline: tokio::time::Instant,
    ) -> DomainResult<ProcessRunOutput> {
        let now = tokio::time::Instant::now();
        let maximum_execution_deadline = now
            .checked_add(command.timeout)
            .ok_or_else(|| unavailable(&self.backend_id))?;
        let execution_deadline = completion_deadline
            .checked_sub(KILL_DEADLINE)
            .filter(|deadline| *deadline > now)
            .ok_or_else(|| unavailable(&self.backend_id))?
            .min(maximum_execution_deadline);
        helper_output(
            self.run_before(
                connection,
                command,
                execution_deadline,
                Some(completion_deadline),
            )
            .await,
            &self.backend_id,
        )
    }
}

fn helper_output(
    result: DomainResult<ProcessRunOutput>,
    backend_id: &str,
) -> DomainResult<ProcessRunOutput> {
    match result {
        Ok(output) if output.exited && output.exit_code.is_some() => Ok(output),
        Err(error) => Err(error),
        Ok(_) => Err(unavailable(backend_id)),
    }
}

fn unavailable(backend_id: &str) -> DomainError {
    DomainError::BackendUnavailable {
        backend_id: backend_id.to_owned(),
    }
}
