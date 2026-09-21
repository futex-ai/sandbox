//! Disposable authenticated ingress smoke for one non-envd port.

use std::{io, time::Duration};

use sandbox_e2b::E2bSandboxBackend;
use sandbox_interface::{
    BackendInputRequest, BackendPortIngressRequest, ResourceOwner, SandboxBackend,
};

use super::support::{
    LiveResources, LiveResult, ingress_client, sandbox_request, terminal_request, wait_for_output,
};

const PORT: u16 = 4173;

trait LiveStage<T> {
    fn stage(self, stage: &'static str) -> LiveResult<T>;
}

impl<T, E> LiveStage<T> for Result<T, E>
where
    E: std::fmt::Debug,
{
    fn stage(self, stage: &'static str) -> LiveResult<T> {
        self.map_err(|error| io::Error::other(format!("{stage}: {error:?}")).into())
    }
}

pub(super) async fn run(
    backend: &E2bSandboxBackend,
    owner: ResourceOwner,
    resources: &mut LiveResources,
) -> LiveResult<()> {
    let sandbox = backend
        .create_sandbox(sandbox_request(owner, None))
        .await
        .stage("sandbox create")?;
    resources.sandboxes.push(sandbox.provider_ref.clone());
    let terminal = backend
        .create_terminal(terminal_request(sandbox.provider_ref.clone()))
        .await
        .stage("terminal create")?;
    resources.track_terminal(&sandbox.provider_ref, &terminal);
    backend
        .write_terminal(BackendInputRequest {
            sandbox_provider_ref: sandbox.provider_ref.clone(),
            terminal_provider_ref: terminal.provider_ref.clone(),
            input: format!(
                "printf sandbox-private-ingress > /tmp/index.html\ncd /tmp\npython3 -m http.server {PORT} --bind 0.0.0.0 >/tmp/sandbox-ingress.log 2>&1 &\necho sandbox-ingress-started\n"
            )
            .into_bytes(),
        })
        .await
        .stage("server start input")?;
    wait_for_output(
        backend,
        sandbox.provider_ref.clone(),
        &terminal,
        0,
        b"sandbox-ingress-started",
    )
    .await
    .stage("server start output")?;

    let ingress = backend
        .port_ingress(BackendPortIngressRequest {
            sandbox_provider_ref: sandbox.provider_ref,
            port: PORT,
        })
        .await
        .stage("port ingress resolve")?;
    let credential = ingress
        .credential()
        .ok_or_else(|| io::Error::other("E2B omitted the private traffic credential"))?;
    let client = ingress_client()?;

    let mut authenticated_status = reqwest::StatusCode::BAD_GATEWAY;
    let mut body = String::new();
    for _ in 0..20 {
        let response = client
            .get(ingress.upstream_url())
            .header(credential.header_name(), credential.header_value())
            .send()
            .await?;
        authenticated_status = response.status();
        if authenticated_status == reqwest::StatusCode::OK {
            body = response.text().await?;
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    if authenticated_status != reqwest::StatusCode::OK || body != "sandbox-private-ingress" {
        return Err(
            io::Error::other("authenticated E2B port ingress did not reach the app").into(),
        );
    }

    let unauthenticated = client.get(ingress.upstream_url()).send().await?;
    if unauthenticated.status() != reqwest::StatusCode::FORBIDDEN {
        return Err(io::Error::other("private E2B port accepted unauthenticated traffic").into());
    }
    Ok(())
}
