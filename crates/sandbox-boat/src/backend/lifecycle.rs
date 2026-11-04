//! One-mutation lifecycle and exact recover-only reconciliation.

use sandbox_interface::{
    BackendCreateSandboxRequest, BackendManagedSandbox, BackendSandbox, Error, ProviderRef,
    ResourceKind, Result, SandboxConsumer,
};

use crate::control::types::BoatLabels;

use super::{configured::BoatSandboxBackend, identity, mapping, validation};

pub(super) async fn create(
    backend: &BoatSandboxBackend,
    request: BackendCreateSandboxRequest,
) -> Result<BackendSandbox> {
    let request = validation::create(backend, &request)?;
    let value = mapping::control_result(
        backend.control.create_sandbox(request).await,
        backend.config.backend_id(),
        Some(ResourceKind::Sandbox),
    )?;
    Ok(mapping::sandbox(value))
}

pub(super) async fn recover(
    backend: &BoatSandboxBackend,
    request: BackendCreateSandboxRequest,
) -> Result<Option<BackendSandbox>> {
    let expected = validation::create(backend, &request)?;
    let candidates = mapping::control_result(
        backend
            .control
            .list_sandboxes(BoatLabels::from([(
                identity::OPERATION.to_owned(),
                request.operation_id.to_string(),
            )]))
            .await,
        backend.config.backend_id(),
        Some(ResourceKind::Sandbox),
    )?;
    let mut matches = candidates
        .into_iter()
        .filter(|row| {
            row.labels.get(identity::OPERATION) == Some(&request.operation_id.to_string())
        })
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err(Error::internal_message(
            "multiple Boat sandboxes matched one operation",
        ));
    }
    let Some(candidate) = matches.pop() else {
        return Ok(None);
    };
    if candidate.labels.get(identity::NETWORK) != expected.labels.get(identity::NETWORK) {
        return Err(Error::SandboxNetworkPolicyMismatch);
    }
    if expected
        .labels
        .iter()
        .any(|(key, value)| candidate.labels.get(key) != Some(value))
        || candidate.machine_type.as_deref() != Some(&expected.machine_type)
        || candidate.ttl_seconds != Some(expected.ttl_seconds)
        || candidate.snapshots == Some(true)
        || candidate.monitoring == Some(true)
    {
        return Err(Error::internal_message(
            "Boat recovery request identity conflicts",
        ));
    }
    Ok(Some(mapping::sandbox(candidate)))
}

pub(super) async fn managed(
    backend: &BoatSandboxBackend,
    deployment_id: String,
) -> Result<Vec<BackendManagedSandbox>> {
    let labels = BoatLabels::from([(identity::DEPLOYMENT.to_owned(), deployment_id.clone())]);
    let rows = mapping::control_result(
        backend.control.list_sandboxes(labels).await,
        backend.config.backend_id(),
        Some(ResourceKind::Sandbox),
    )?;
    Ok(rows
        .into_iter()
        .filter(|row| row.labels.get(identity::DEPLOYMENT) == Some(&deployment_id))
        .map(|row| BackendManagedSandbox {
            provider_ref: ProviderRef::new(row.id),
            state: mapping::sandbox_state(row.state),
            sandbox_id: row
                .labels
                .get(identity::SANDBOX)
                .and_then(|value| value.parse().ok()),
            operation_id: row
                .labels
                .get(identity::OPERATION)
                .and_then(|value| value.parse().ok()),
            consumer: row
                .labels
                .get(identity::CONSUMER)
                .and_then(|value| SandboxConsumer::from_metadata(value)),
            lifetime: identity::lifetime(&row.labels),
        })
        .collect())
}
