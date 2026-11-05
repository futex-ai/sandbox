//! Pure local create validation before any Boat dispatch or recovery read.

use sandbox_interface::{
    BackendCreateSandboxRequest, Error, ResourceKind, Result, SandboxConsumer, SandboxLifetime,
    SandboxNetworkPolicy,
};

use crate::{
    catalogue, config::canonical, control::types::BoatCreateSandbox,
    transport::request::valid_snapshot_name,
};

use super::{configured::BoatSandboxBackend, identity};

pub(super) fn create(
    backend: &BoatSandboxBackend,
    request: &BackendCreateSandboxRequest,
) -> Result<BoatCreateSandbox> {
    if request.network != SandboxNetworkPolicy::Open {
        return Err(Error::UnsupportedNetworkPolicy {
            policy: request.network.clone(),
        });
    }
    request.lifetime.validate()?;
    let SandboxLifetime::OneShot { max_lifetime } = request.lifetime else {
        return Err(Error::internal_message(
            "Boat requires a one-shot sandbox lifetime",
        ));
    };
    if max_lifetime.as_secs() > 2_592_000 {
        return Err(Error::InvalidSeconds {
            field: "max_lifetime",
            minimum: 1,
            maximum: 2_592_000,
        });
    }
    let profile = backend
        .config
        .profile(&request.profile)
        .ok_or(Error::UnknownProfile)?;
    let machine_type = match &request.size {
        None => profile.provider_size.clone(),
        Some(selection) => {
            catalogue::select(selection).ok_or_else(|| Error::UnsupportedSizeSelection {
                selection: selection.clone(),
            })?
        }
    };
    if !canonical(&request.deployment_id) || request.deployment_id.len() > 256 {
        return Err(Error::internal_message(
            "Boat deployment identity is invalid",
        ));
    }
    if let Some(source) = &request.snapshot_provider_ref {
        if request.consumer == SandboxConsumer::Browser {
            return Err(Error::RuntimeConsumerRequired {
                resource: ResourceKind::Snapshot,
                consumer: request.consumer,
            });
        }
        if !valid_snapshot_name(source.as_str()) {
            return Err(Error::internal_message(
                "Boat saved-snapshot reference is invalid",
            ));
        }
    }
    let ttl_seconds = max_lifetime.as_secs() as u32;
    Ok(BoatCreateSandbox {
        labels: identity::labels(
            request,
            backend.config.backend_id(),
            &machine_type,
            ttl_seconds,
        ),
        machine_type,
        ttl_seconds,
        no_env: true,
        snapshots: false,
        monitoring: false,
        from: request
            .snapshot_provider_ref
            .as_ref()
            .map(|source| source.as_str().to_owned()),
        operation: request.operation_id.to_string(),
    })
}
