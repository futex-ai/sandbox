//! Provider-neutral Boat lifecycle, snapshot, and failure mappings.

use sandbox_interface::{
    BackendSandbox, BackendSnapshot, Error, ProviderRef, ResourceKind, Result, SandboxState,
    SnapshotState,
};

use crate::{
    control::types::{BoatSandbox, BoatSandboxState, BoatSavedSnapshot, BoatSavedSnapshotStatus},
    error::{Error as AdapterError, Result as AdapterResult},
};

pub(super) fn control_result<T>(
    result: AdapterResult<T>,
    backend_id: &str,
    resource: Option<ResourceKind>,
) -> Result<T> {
    match result {
        Ok(value) => Ok(value),
        Err(AdapterError::NotFound) => Err(Error::NotFound {
            resource: resource.unwrap_or(ResourceKind::Sandbox),
        }),
        Err(AdapterError::StartRefused { refusal }) => Err(Error::SandboxStartRefused {
            backend_id: backend_id.to_owned(),
            refusal,
        }),
        Err(
            AdapterError::Unavailable
            | AdapterError::DeliveryAmbiguous
            | AdapterError::InvalidResponse
            | AdapterError::InvalidPagination
            | AdapterError::ResponseTooLarge,
        ) => Err(Error::BackendUnavailable {
            backend_id: backend_id.to_owned(),
        }),
        Err(error) => Err(Error::internal_with(error, "Boat control adapter")),
    }
}

pub(super) fn sandbox(value: BoatSandbox) -> BackendSandbox {
    BackendSandbox {
        provider_ref: ProviderRef::new(value.id),
        state: sandbox_state(value.state),
    }
}

pub(super) fn sandbox_state(state: BoatSandboxState) -> SandboxState {
    match state {
        BoatSandboxState::Init
        | BoatSandboxState::Provisioning
        | BoatSandboxState::Provisioned
        | BoatSandboxState::Cloning => SandboxState::Creating,
        BoatSandboxState::Ready | BoatSandboxState::Idle | BoatSandboxState::Running => {
            SandboxState::Ready
        }
        BoatSandboxState::Archiving => SandboxState::Destroying,
        BoatSandboxState::Archived | BoatSandboxState::Cancelled => SandboxState::Destroyed,
        BoatSandboxState::Error => SandboxState::Failed,
    }
}

pub(super) fn snapshot(value: BoatSavedSnapshot) -> BackendSnapshot {
    let state = match value.status {
        BoatSavedSnapshotStatus::Saving => SnapshotState::Creating,
        BoatSavedSnapshotStatus::Ready => SnapshotState::Ready,
        BoatSavedSnapshotStatus::Failed => SnapshotState::Failed,
    };
    BackendSnapshot {
        provider_ref: ProviderRef::new(value.name),
        state,
    }
}
