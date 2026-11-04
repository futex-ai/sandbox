//! Exact durable create identity preserved in provider labels.

use sandbox_interface::{BackendCreateSandboxRequest, SandboxLifetime, SandboxSizeSelection};

use crate::control::types::BoatLabels;

pub(super) const DEPLOYMENT: &str = "sandbox.deployment";
pub(super) const OPERATION: &str = "sandbox.operation";
pub(super) const SANDBOX: &str = "sandbox.id";
pub(super) const CONSUMER: &str = "sandbox.consumer";
pub(super) const LIFETIME: &str = "sandbox.lifetime";
pub(super) const TTL: &str = "sandbox.ttl";
pub(super) const NETWORK: &str = "sandbox.network";

pub(super) fn labels(
    request: &BackendCreateSandboxRequest,
    backend: &str,
    size: &str,
    ttl: u32,
) -> BoatLabels {
    BoatLabels::from([
        ("sandbox.version".to_owned(), "1".to_owned()),
        ("sandbox.backend".to_owned(), backend.to_owned()),
        (DEPLOYMENT.to_owned(), request.deployment_id.clone()),
        (OPERATION.to_owned(), request.operation_id.to_string()),
        (SANDBOX.to_owned(), request.sandbox_id.to_string()),
        (
            "sandbox.workspace".to_owned(),
            request.owner.workspace_id.to_string(),
        ),
        (
            "sandbox.agent".to_owned(),
            request
                .owner
                .agent_id
                .map_or_else(|| "none".to_owned(), |id| id.to_string()),
        ),
        (CONSUMER.to_owned(), request.consumer.as_str().to_owned()),
        ("sandbox.profile".to_owned(), request.profile.clone()),
        (
            "sandbox.requested-size".to_owned(),
            requested_size(&request.size),
        ),
        ("sandbox.size".to_owned(), size.to_owned()),
        (NETWORK.to_owned(), "open".to_owned()),
        (LIFETIME.to_owned(), "one_shot".to_owned()),
        (TTL.to_owned(), ttl.to_string()),
        (
            "sandbox.from".to_owned(),
            request.snapshot_provider_ref.as_ref().map_or_else(
                || "none".to_owned(),
                |source| format!("saved:{}", source.as_str()),
            ),
        ),
    ])
}

pub(super) fn lifetime(labels: &BoatLabels) -> Option<SandboxLifetime> {
    let lifetime =
        SandboxLifetime::from_metadata(labels.get(LIFETIME)?, labels.get(TTL).map(String::as_str))?;
    match lifetime {
        SandboxLifetime::OneShot { max_lifetime } if max_lifetime.as_secs() <= 2_592_000 => {
            Some(lifetime)
        }
        _ => None,
    }
}

fn requested_size(selection: &Option<SandboxSizeSelection>) -> String {
    match selection {
        None => "profile".to_owned(),
        Some(SandboxSizeSelection::ProviderSize(name)) => format!("provider:{name}"),
        Some(SandboxSizeSelection::Resources(resources)) => format!(
            "resources:{}:{}:{:?}:{}:{:?}",
            resources.vcpus,
            resources.memory.amount,
            resources.memory.unit,
            resources.disk.amount,
            resources.disk.unit
        ),
    }
}
