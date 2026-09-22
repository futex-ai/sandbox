//! E2B sandbox lifecycle and operation-correlation mapping.

use std::collections::BTreeMap;

use sandbox_interface::{
    BackendCreateSandboxRequest, BackendManagedSandbox, BackendSandbox, Error, ProviderRef,
    ResourceKind, Result, SandboxConsumer, SandboxNetworkPolicy,
};

use crate::{
    config::E2bProfile,
    control::{ControlCreateSandbox, ControlSandbox, SandboxMetadata},
    error::Error as AdapterError,
    runtime_conventions::E2bRuntimeConventions,
};

use super::{configured::E2bSandboxBackend, mapping, sandbox_metadata};

pub(super) async fn managed(
    backend: &E2bSandboxBackend,
    deployment_id: String,
) -> Result<Vec<BackendManagedSandbox>> {
    let conventions = backend.config.runtime_conventions();
    let metadata = BTreeMap::from([(conventions.metadata_key("deployment_id"), deployment_id)]);
    Ok(
        control_result(backend, backend.control.list_sandboxes(metadata).await)?
            .into_iter()
            .map(|sandbox| managed_sandbox(sandbox, conventions))
            .collect(),
    )
}

fn managed_sandbox(
    sandbox: ControlSandbox,
    conventions: &E2bRuntimeConventions,
) -> BackendManagedSandbox {
    BackendManagedSandbox {
        provider_ref: ProviderRef::new(sandbox.sandbox_id),
        state: mapping::sandbox_state(sandbox.state),
        sandbox_id: metadata_id(&sandbox.metadata, &conventions.metadata_key("sandbox_id")),
        operation_id: metadata_id(&sandbox.metadata, &conventions.metadata_key("operation_id")),
        consumer: sandbox
            .metadata
            .get(&conventions.metadata_key("consumer"))
            .and_then(|value| SandboxConsumer::from_metadata(value)),
        lifetime: sandbox_metadata::lifetime(&sandbox.metadata, conventions),
    }
}

fn metadata_id<T>(metadata: &SandboxMetadata, key: &str) -> Option<T>
where
    T: std::str::FromStr,
{
    metadata.get(key)?.parse().ok()
}

pub(super) async fn create(
    backend: &E2bSandboxBackend,
    request: BackendCreateSandboxRequest,
) -> Result<BackendSandbox> {
    let profile = validate_request(backend, &request)?;
    let metadata = sandbox_metadata::for_create(backend.config.runtime_conventions(), &request);
    let existing = control_result(
        backend,
        backend.control.list_sandboxes(metadata.clone()).await,
    )?;
    if let Some(existing) = exactly_one(existing)? {
        return Ok(map_sandbox(existing));
    }
    let template_id = request.snapshot_provider_ref.map_or_else(
        || profile.template.clone(),
        |reference| reference.as_str().to_owned(),
    );
    let created = backend
        .control
        .create_sandbox(ControlCreateSandbox {
            template_id,
            metadata: metadata.clone(),
            allow_public_egress: profile.allow_public_egress,
            denied_destinations: profile.denied_destinations.clone(),
            idle_timeout_seconds: backend.config.idle_timeout_seconds(),
            lifetime: request.lifetime,
        })
        .await;
    match created {
        Ok(created) => Ok(BackendSandbox {
            provider_ref: ProviderRef::new(created.sandbox_id),
            state: sandbox_interface::SandboxState::Ready,
        }),
        Err(AdapterError::DeliveryAmbiguous) => {
            let recovered =
                control_result(backend, backend.control.list_sandboxes(metadata).await)?;
            exactly_one(recovered)?
                .map(map_sandbox)
                .ok_or_else(|| Error::BackendUnavailable {
                    backend_id: backend.config.backend_id().to_owned(),
                })
        }
        Err(error) => Err(map_control(backend, error)),
    }
}

pub(super) async fn recover(
    backend: &E2bSandboxBackend,
    request: BackendCreateSandboxRequest,
) -> Result<Option<BackendSandbox>> {
    validate_request(backend, &request)?;
    let existing = control_result(
        backend,
        backend
            .control
            .list_sandboxes(sandbox_metadata::for_create(
                backend.config.runtime_conventions(),
                &request,
            ))
            .await,
    )?;
    Ok(exactly_one(existing)?.map(map_sandbox))
}

fn validate_request<'a>(
    backend: &'a E2bSandboxBackend,
    request: &BackendCreateSandboxRequest,
) -> Result<&'a E2bProfile> {
    request.lifetime.validate()?;
    validate_network(request.network)?;
    backend
        .config
        .profile(&request.profile)
        .ok_or(Error::UnknownProfile)
}

/// `open` applies no additional per-session restriction; the deployment-owned
/// profile egress policy is threaded unchanged. Any other typed policy fails
/// before any provider dispatch.
fn validate_network(network: SandboxNetworkPolicy) -> Result<()> {
    match network {
        SandboxNetworkPolicy::Open => Ok(()),
        unsupported => Err(Error::UnsupportedNetworkPolicy {
            policy: unsupported,
        }),
    }
}

pub(super) async fn inspect(
    backend: &E2bSandboxBackend,
    provider_ref: ProviderRef,
) -> Result<BackendSandbox> {
    let sandbox = control_result(
        backend,
        backend.control.get_sandbox(provider_ref.as_str()).await,
    )?;
    mapping::ensure_sandbox_identity(&provider_ref, &sandbox.sandbox_id)?;
    Ok(BackendSandbox {
        provider_ref,
        state: mapping::sandbox_state(sandbox.state),
    })
}

pub(super) async fn resume(
    backend: &E2bSandboxBackend,
    provider_ref: ProviderRef,
) -> Result<BackendSandbox> {
    let access = control_result(
        backend,
        backend.control.connect_sandbox(provider_ref.as_str()).await,
    )?;
    mapping::ensure_sandbox_identity(&provider_ref, &access.sandbox_id)?;
    Ok(BackendSandbox {
        provider_ref,
        state: sandbox_interface::SandboxState::Ready,
    })
}

pub(super) async fn pause(
    backend: &E2bSandboxBackend,
    provider_ref: ProviderRef,
) -> Result<BackendSandbox> {
    control_result(
        backend,
        backend.control.pause_sandbox(provider_ref.as_str()).await,
    )?;
    Ok(BackendSandbox {
        provider_ref,
        state: sandbox_interface::SandboxState::Paused,
    })
}

pub(super) async fn destroy(backend: &E2bSandboxBackend, provider_ref: ProviderRef) -> Result<()> {
    control_result(
        backend,
        backend.control.kill_sandbox(provider_ref.as_str()).await,
    )
}

fn exactly_one(mut sandboxes: Vec<ControlSandbox>) -> Result<Option<ControlSandbox>> {
    match sandboxes.len() {
        0 => Ok(None),
        1 => Ok(sandboxes.pop()),
        _ => Err(Error::internal_message(
            "multiple E2B sandboxes matched one consumer operation",
        )),
    }
}

fn map_sandbox(sandbox: ControlSandbox) -> BackendSandbox {
    BackendSandbox {
        provider_ref: ProviderRef::new(sandbox.sandbox_id),
        state: mapping::sandbox_state(sandbox.state),
    }
}

fn map_control(backend: &E2bSandboxBackend, error: AdapterError) -> Error {
    mapping::control(
        error,
        backend.config.backend_id(),
        Some(ResourceKind::Sandbox),
    )
}

fn control_result<T>(backend: &E2bSandboxBackend, result: crate::error::Result<T>) -> Result<T> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => Err(map_control(backend, error)),
    }
}
