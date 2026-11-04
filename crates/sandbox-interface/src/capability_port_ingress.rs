//! Call-local authenticated ingress to one exact sandbox port.

use async_trait::async_trait;

use crate::{BackendPortIngressRequest, PortIngress, Result};

/// Call-local authenticated ingress to one exact sandbox port.
#[unimock::unimock(api = SandboxPortIngressMock)]
#[async_trait]
pub trait SandboxPortIngress: Send + Sync {
    /// Resolves one exact port into a call-local authenticated upstream.
    async fn port_ingress(&self, request: BackendPortIngressRequest) -> Result<PortIngress>;
}
