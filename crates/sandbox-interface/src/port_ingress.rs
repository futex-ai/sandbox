//! Call-local provider-neutral HTTP port-ingress contract.

use std::fmt;

use crate::{ProviderRef, ResourceOwner, SandboxId};

/// Provider request to resolve one sandbox port into an authenticated upstream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendPortIngressRequest {
    /// Source provider sandbox reference.
    pub sandbox_provider_ref: ProviderRef,
    /// Exact HTTP port inside the sandbox.
    pub port: u16,
}

/// Authorized request to resolve one owned sandbox port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortIngressRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Stable sandbox handle.
    pub sandbox_id: SandboxId,
    /// Exact HTTP port inside the sandbox.
    pub port: u16,
}

/// Optional call-local header credential required by a sandbox upstream.
#[derive(Clone, Eq, PartialEq)]
pub struct PortIngressCredential {
    header_name: String,
    header_value: String,
}

impl PortIngressCredential {
    /// Creates one header credential without persisting or serializing it.
    #[must_use]
    pub fn new(header_name: impl Into<String>, header_value: impl Into<String>) -> Self {
        Self {
            header_name: header_name.into(),
            header_value: header_value.into(),
        }
    }

    /// Borrows the provider-defined header name.
    #[must_use]
    pub fn header_name(&self) -> &str {
        &self.header_name
    }

    /// Borrows the secret header value for the immediate upstream request.
    #[must_use]
    pub fn header_value(&self) -> &str {
        &self.header_value
    }
}

impl fmt::Debug for PortIngressCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PortIngressCredential")
            .field("header_name", &self.header_name)
            .field("header_value", &"[redacted]")
            .finish()
    }
}

/// Call-local upstream for one exact sandbox port.
#[derive(Clone, Eq, PartialEq)]
pub struct PortIngress {
    upstream_url: String,
    credential: Option<PortIngressCredential>,
}

impl PortIngress {
    /// Creates an upstream that must remain inside trusted runtime composition.
    #[must_use]
    pub fn new(upstream_url: impl Into<String>, credential: Option<PortIngressCredential>) -> Self {
        Self {
            upstream_url: upstream_url.into(),
            credential,
        }
    }

    /// Borrows the exact provider upstream URL.
    #[must_use]
    pub fn upstream_url(&self) -> &str {
        &self.upstream_url
    }

    /// Borrows the optional immediate-request transport credential.
    #[must_use]
    pub const fn credential(&self) -> Option<&PortIngressCredential> {
        self.credential.as_ref()
    }
}

impl fmt::Debug for PortIngress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PortIngress")
            .field("upstream_url", &"[redacted]")
            .field("credential", &self.credential)
            .finish()
    }
}

#[cfg(test)]
#[path = "_tests_/port_ingress_tests.rs"]
mod port_ingress_tests;
