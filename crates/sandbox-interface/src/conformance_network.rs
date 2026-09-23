//! Shared deny-by-default outbound allowlist conformance probe.

use std::{collections::BTreeMap, time::Duration};

use crate::{
    BackendCreateSandboxRequest, BackendRunProcessRequest, EgressDestination, Error, OperationId,
    ProviderRef, ResourceOwner, Result, SandboxBackend, SandboxConsumer, SandboxId,
    SandboxLifetime, SandboxNetworkPolicy, SandboxProcessOutput,
    conformance_resources::ConformanceResources,
};

const ALLOWED_FETCH: &str = "curl --disable --noproxy '*' --fail --silent --show-error --connect-timeout 10 --max-time 15 --output /dev/null https://example.com/";
const DISALLOWED_FETCH: &str = "curl --disable --noproxy '*' --silent --show-error --connect-timeout 10 --max-time 15 --output /dev/null https://www.google.com/";

pub(crate) async fn exercise(
    backend: &dyn SandboxBackend,
    resources: &mut ConformanceResources<'_>,
    owner: ResourceOwner,
    profile: &str,
) -> Result<()> {
    let request = BackendCreateSandboxRequest {
        sandbox_id: SandboxId::new(),
        operation_id: OperationId::new(),
        owner,
        consumer: SandboxConsumer::Runtime,
        lifetime: SandboxLifetime::IdleAutoPause,
        deployment_id: "backend-conformance".to_owned(),
        profile: profile.to_owned(),
        network: SandboxNetworkPolicy::allowlist(vec![EgressDestination::domain("example.com")?])?,
        snapshot_provider_ref: None,
    };
    let sandbox = resources.create_sandbox(request.clone()).await?;
    assert_mismatched_recovery_fails(backend, request).await?;

    let allowed = fetch(backend, &sandbox.provider_ref, ALLOWED_FETCH).await?;
    if !allowed.exited
        || allowed.exit_code != Some(0)
        || allowed.stdout_overflowed
        || allowed.stderr_overflowed
    {
        return Err(Error::internal_message(
            "backend allowlist blocked its exact allowed domain",
        ));
    }

    let disallowed = fetch(backend, &sandbox.provider_ref, DISALLOWED_FETCH).await?;
    if !disallowed.exited
        || disallowed.exit_code.is_none_or(|code| code == 0)
        || disallowed.stdout_overflowed
        || disallowed.stderr_overflowed
    {
        return Err(Error::internal_message(
            "backend allowlist returned an application response from a disallowed domain",
        ));
    }
    Ok(())
}

async fn assert_mismatched_recovery_fails(
    backend: &dyn SandboxBackend,
    mut request: BackendCreateSandboxRequest,
) -> Result<()> {
    request.network =
        SandboxNetworkPolicy::allowlist(vec![EgressDestination::domain("different.example.com")?])?;
    match backend.recover_sandbox_create(request).await {
        Err(Error::SandboxNetworkPolicyMismatch) => Ok(()),
        _ => Err(Error::internal_message(
            "backend sandbox recovery accepted a different network policy",
        )),
    }
}

async fn fetch(
    backend: &dyn SandboxBackend,
    sandbox_provider_ref: &ProviderRef,
    script: &str,
) -> Result<SandboxProcessOutput> {
    backend
        .run_process(BackendRunProcessRequest {
            sandbox_provider_ref: sandbox_provider_ref.clone(),
            command: "/bin/sh".to_owned(),
            args: vec!["-c".to_owned(), script.to_owned()],
            cwd: None,
            envs: BTreeMap::new(),
            stdout_limit: 4096,
            stderr_limit: 4096,
            deadline: Duration::from_secs(30),
        })
        .await
}

#[cfg(test)]
#[path = "_tests_/conformance_network_tests.rs"]
mod conformance_network_tests;
