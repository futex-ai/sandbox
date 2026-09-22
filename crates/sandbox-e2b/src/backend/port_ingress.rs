//! Authenticated E2B routing for one exact non-envd sandbox port.

use sandbox_interface::{
    BackendPortIngressRequest, Error, PortIngress, PortIngressCredential, ResourceKind, Result,
};
use url::Url;

use super::{configured::E2bSandboxBackend, mapping};

const TRAFFIC_ACCESS_HEADER: &str = "e2b-traffic-access-token";

pub(super) async fn resolve(
    backend: &E2bSandboxBackend,
    request: BackendPortIngressRequest,
) -> Result<PortIngress> {
    if request.port == 0 {
        return Err(Error::InvalidPort);
    }
    let access = mapping::control_result(
        backend
            .control
            .connect_sandbox(request.sandbox_provider_ref.as_str())
            .await,
        backend.config.backend_id(),
        Some(ResourceKind::Sandbox),
    )?;
    mapping::ensure_sandbox_identity(&request.sandbox_provider_ref, &access.sandbox_id)?;
    let Some(traffic_access_token) = access.traffic_access_token else {
        return Err(Error::BackendUnavailable {
            backend_id: backend.config.backend_id().to_owned(),
        });
    };
    let expected_host = format!(
        "{}-{}.{}",
        request.port,
        access.sandbox_id,
        backend.config.sandbox_domain()
    );
    let upstream = format!("https://{expected_host}");
    let upstream = match Url::parse(&upstream) {
        Ok(upstream)
            if upstream.scheme() == "https"
                && upstream.host_str() == Some(expected_host.as_str())
                && upstream.path() == "/"
                && upstream.query().is_none()
                && upstream.fragment().is_none() =>
        {
            upstream
        }
        Ok(_) => {
            return Err(Error::internal_message(
                "E2B port ingress produced an unsafe upstream URL",
            ));
        }
        Err(source) => {
            return Err(Error::internal_with(
                source,
                "parse E2B port ingress upstream URL",
            ));
        }
    };
    Ok(PortIngress::new(
        upstream.to_string(),
        Some(PortIngressCredential::new(
            TRAFFIC_ACCESS_HEADER,
            traffic_access_token,
        )),
    ))
}
