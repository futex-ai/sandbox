//! Live private-ingress checks for both screen bridges.

use std::{io, time::Duration};

use sandbox_e2b::E2bSandboxBackend;
use sandbox_interface::{
    BackendEnsureScreenStackRequest, BackendPortIngressRequest, ResourceOwner, SandboxBackend,
    ScreenStackOutcome,
};

use super::support::{
    LiveResources, LiveResult, create_tracked_sandbox, ingress_client, sandbox_request,
};

const SCREEN_PORTS: [u16; 2] = [6080, 6081];

pub(super) async fn run(
    backend: &E2bSandboxBackend,
    owner: ResourceOwner,
    resources: &mut LiveResources,
) -> LiveResult<()> {
    let sandbox = create_tracked_sandbox(backend, sandbox_request(owner, None), resources).await?;
    let outcome = backend
        .ensure_screen_stack(BackendEnsureScreenStackRequest {
            sandbox_provider_ref: sandbox.provider_ref.clone(),
        })
        .await?;
    if !matches!(outcome, ScreenStackOutcome::Ready { .. }) {
        return Err(io::Error::other("screen template did not report a ready stack").into());
    }
    let client = ingress_client()?;
    for port in SCREEN_PORTS {
        assert_private_bridge(backend, &client, sandbox.provider_ref.clone(), port).await?;
    }
    Ok(())
}

async fn assert_private_bridge(
    backend: &E2bSandboxBackend,
    client: &reqwest::Client,
    sandbox_provider_ref: sandbox_interface::ProviderRef,
    port: u16,
) -> LiveResult<()> {
    let ingress = backend
        .port_ingress(BackendPortIngressRequest {
            sandbox_provider_ref,
            port,
        })
        .await?;
    let unauthenticated = client.get(ingress.upstream_url()).send().await?;
    if unauthenticated.status() != reqwest::StatusCode::FORBIDDEN {
        return Err(io::Error::other(format!(
            "screen bridge {port} accepted unauthenticated traffic"
        ))
        .into());
    }
    let credential = ingress
        .credential()
        .ok_or_else(|| io::Error::other("E2B omitted the private traffic credential"))?;
    for _ in 0..20 {
        let response = client
            .get(ingress.upstream_url())
            .header(credential.header_name(), credential.header_value())
            .send()
            .await?;
        if response.status() != reqwest::StatusCode::FORBIDDEN
            && !response.status().is_server_error()
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(io::Error::other(format!(
        "authenticated screen bridge {port} was not reachable"
    ))
    .into())
}
