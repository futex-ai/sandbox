//! E2B control-plane to provider-neutral state and error mapping.

use sandbox_interface::{
    Error as DomainError, ProviderRef, ResourceKind, SandboxState, SnapshotState, TerminalState,
};

use crate::{
    control::ControlSandboxState,
    error::Error,
    process::{ProcessConnection, ProcessInfo},
};

use super::configured::E2bSandboxBackend;

pub(super) fn control(
    error: Error,
    backend_id: &str,
    resource: Option<ResourceKind>,
) -> DomainError {
    match error {
        Error::NotFound => DomainError::NotFound {
            resource: resource.unwrap_or(ResourceKind::Sandbox),
        },
        Error::Unavailable | Error::DeliveryAmbiguous => DomainError::BackendUnavailable {
            backend_id: backend_id.to_owned(),
        },
        error => DomainError::internal_with(error, "E2B control adapter"),
    }
}

pub(super) fn control_result<T>(
    result: crate::error::Result<T>,
    backend_id: &str,
    resource: Option<ResourceKind>,
) -> Result<T, DomainError> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => Err(control(error, backend_id, resource)),
    }
}

pub(super) fn sandbox_state(state: ControlSandboxState) -> SandboxState {
    match state {
        ControlSandboxState::Running => SandboxState::Ready,
        ControlSandboxState::Paused => SandboxState::Paused,
    }
}

pub(super) fn ready_snapshot(provider_ref: String) -> sandbox_interface::BackendSnapshot {
    sandbox_interface::BackendSnapshot {
        provider_ref: sandbox_interface::ProviderRef::new(provider_ref),
        state: SnapshotState::Ready,
    }
}

pub(super) fn terminal_state(process: Option<&ProcessInfo>) -> TerminalState {
    if process.is_some() {
        TerminalState::Ready
    } else {
        TerminalState::Exited
    }
}

pub(super) fn ensure_sandbox_identity(
    expected: &ProviderRef,
    actual: &str,
) -> Result<(), DomainError> {
    if expected.as_str() != actual {
        return Err(DomainError::internal_message(
            "E2B existing sandbox response identity mismatch",
        ));
    }
    Ok(())
}

pub(super) async fn connection(
    backend: &E2bSandboxBackend,
    sandbox_ref: &ProviderRef,
) -> Result<ProcessConnection, DomainError> {
    let access = control_result(
        backend.control.connect_sandbox(sandbox_ref.as_str()).await,
        &backend.config.backend_id,
        Some(ResourceKind::Sandbox),
    )?;
    ensure_sandbox_identity(sandbox_ref, &access.sandbox_id)?;
    Ok(ProcessConnection::new(
        sandbox_ref.as_str().to_owned(),
        access.domain,
        access.envd_access_token,
    ))
}

/// Acquires envd credentials with `GET /sandboxes/{sandboxID}` only. E2B's
/// `POST /sandboxes/{sandboxID}/connect` resumes paused sandboxes and extends
/// TTL, so read-only execution never calls it. The GET must report `running`
/// with `lifecycle.autoResume=false`; direct envd process RPCs carry no
/// sandbox-timeout parameter and cannot opt into lifecycle extension.
pub(super) async fn read_only_connection(
    backend: &E2bSandboxBackend,
    sandbox_ref: &ProviderRef,
) -> Result<ProcessConnection, DomainError> {
    let access = control_result(
        backend
            .control
            .get_sandbox_read_access(sandbox_ref.as_str())
            .await,
        &backend.config.backend_id,
        Some(ResourceKind::Sandbox),
    )?;
    ensure_sandbox_identity(sandbox_ref, &access.sandbox_id)?;
    Ok(ProcessConnection::new(
        sandbox_ref.as_str().to_owned(),
        access.domain,
        access.envd_access_token,
    ))
}
