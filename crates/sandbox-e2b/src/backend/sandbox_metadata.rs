//! Provider metadata encoding and recovery for sandbox creation.

use std::collections::BTreeMap;

use sandbox_interface::{BackendCreateSandboxRequest, SandboxLifetime};

use crate::{control::SandboxMetadata, runtime_conventions::E2bRuntimeConventions};

pub(super) fn for_create(
    conventions: &E2bRuntimeConventions,
    request: &BackendCreateSandboxRequest,
) -> SandboxMetadata {
    let mut metadata = BTreeMap::from([
        (
            conventions.metadata_key("deployment_id"),
            request.deployment_id.clone(),
        ),
        (
            conventions.metadata_key("operation_id"),
            request.operation_id.to_string(),
        ),
        (
            conventions.metadata_key("sandbox_id"),
            request.sandbox_id.to_string(),
        ),
        (
            conventions.metadata_key("workspace_id"),
            request.owner.workspace_id.to_string(),
        ),
        (
            conventions.metadata_key("consumer"),
            request.consumer.as_str().to_owned(),
        ),
        (
            conventions.metadata_key("lifetime"),
            request.lifetime.metadata_kind().to_owned(),
        ),
    ]);
    if let SandboxLifetime::OneShot { max_lifetime } = request.lifetime {
        metadata.insert(
            conventions.metadata_key("one_shot_max_lifetime_seconds"),
            max_lifetime.as_secs().to_string(),
        );
    }
    match request.owner.agent_id {
        Some(agent_id) => {
            metadata.insert(conventions.metadata_key("agent_id"), agent_id.to_string());
            metadata.insert(conventions.metadata_key("owner_kind"), "agent".to_owned());
        }
        None => {
            metadata.insert(
                conventions.metadata_key("owner_kind"),
                "platform".to_owned(),
            );
        }
    }
    metadata
}

pub(super) fn lifetime(
    metadata: &SandboxMetadata,
    conventions: &E2bRuntimeConventions,
) -> Option<SandboxLifetime> {
    let kind = metadata.get(&conventions.metadata_key("lifetime"))?;
    let seconds = metadata
        .get(&conventions.metadata_key("one_shot_max_lifetime_seconds"))
        .map(String::as_str);
    SandboxLifetime::from_metadata(kind, seconds)
}
