//! Immutable validated Boat origin, credentials, and deployment profiles.

use std::{collections::BTreeMap, fmt};

use sandbox_interface::{SANDBOX_PROFILE_MAX_ITEMS, valid_sandbox_profile_name};
use url::Url;

use crate::{
    catalogue,
    error::{Error, Result},
};

/// Deployment-owned Boat size and network requirements.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoatProfile {
    /// Published default size used when a request has no explicit selection.
    pub provider_size: String,
    /// Boat supports only unrestricted public egress.
    pub allow_public_egress: bool,
    /// Deployment deny rules; Boat rejects nonempty rules it cannot enforce.
    pub denied_destinations: Vec<String>,
}

/// Validated account configuration. Credentials remain outside the workload.
#[derive(Clone, Eq, PartialEq)]
pub struct BoatAdapterConfig {
    backend_id: String,
    api_base: String,
    api_key: String,
    org: Option<String>,
    profiles: BTreeMap<String, BoatProfile>,
    workload_user: String,
}

impl fmt::Debug for BoatAdapterConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoatAdapterConfig")
            .field("backend_id", &self.backend_id)
            .field("api_base", &self.api_base)
            .field("api_key", &"[redacted]")
            .field("org", &self.org)
            .field("profiles", &self.profiles)
            .finish()
    }
}

impl BoatAdapterConfig {
    /// Validates an HTTPS origin or its exact `/api/v1` base and canonical key.
    pub fn new(
        backend_id: impl Into<String>,
        api_base: impl Into<String>,
        api_key: impl Into<String>,
        profiles: BTreeMap<String, BoatProfile>,
    ) -> Result<Self> {
        let backend_id = backend_id.into();
        let api_base = api_base.into();
        let api_key = api_key.into();
        let parsed = match Url::parse(&api_base) {
            Ok(parsed) => parsed,
            Err(_) => return Err(Error::InvalidRequest),
        };
        if !canonical(&backend_id)
            || backend_id.len() > 256
            || !canonical(&api_key)
            || !valid_header(&api_key)
            || !canonical(&api_base)
            || parsed.scheme() != "https"
            || !parsed.has_host()
            || !matches!(parsed.path(), "/" | "/api/v1")
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || profiles.is_empty()
            || profiles.len() > SANDBOX_PROFILE_MAX_ITEMS
            || profiles.iter().any(|(name, profile)| {
                !valid_sandbox_profile_name(name)
                    || catalogue::offer(&profile.provider_size).is_none()
                    || !profile.allow_public_egress
                    || !profile.denied_destinations.is_empty()
            })
        {
            return Err(Error::InvalidRequest);
        }
        Ok(Self {
            backend_id,
            api_base: format!("{}/api/v1", parsed.origin().ascii_serialization()),
            api_key,
            org: None,
            profiles,
            workload_user: "sandbox-workload".to_owned(),
        })
    }

    /// Pins the billing wallet for both capacity and sandbox operations.
    pub fn with_organization(mut self, organization: impl Into<String>) -> Result<Self> {
        let organization = organization.into();
        if !canonical(&organization)
            || !organization
                .bytes()
                .all(|byte| (0x20..=0x7e).contains(&byte))
        {
            return Err(Error::InvalidRequest);
        }
        self.org = Some(organization);
        Ok(self)
    }

    /// Selects a compatible non-root account without sudo or trusted-storage access.
    /// The default restricted account is created inside a normal Boat image.
    pub fn with_workload_user(mut self, user: impl Into<String>) -> Result<Self> {
        let user = user.into();
        if user.is_empty()
            || user.len() > 31
            || matches!(user.as_str(), "root" | "user")
            || !user.as_bytes()[0].is_ascii_lowercase()
            || !user
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
        {
            return Err(Error::InvalidRequest);
        }
        self.workload_user = user;
        Ok(self)
    }

    pub(crate) fn workload_user(&self) -> &str {
        &self.workload_user
    }

    /// Stable backend identity used by the consumer registry.
    #[must_use]
    pub fn backend_id(&self) -> &str {
        &self.backend_id
    }

    /// Exact HTTPS API base, including the fixed version prefix.
    #[must_use]
    pub fn api_base(&self) -> &str {
        &self.api_base
    }

    pub(crate) fn api_key(&self) -> &str {
        &self.api_key
    }
    pub(crate) fn organization(&self) -> Option<&str> {
        self.org.as_deref()
    }
    pub(crate) fn profile(&self, name: &str) -> Option<&BoatProfile> {
        self.profiles.get(name)
    }
}

pub(crate) fn canonical(value: &str) -> bool {
    !value.is_empty() && value.trim() == value
}

fn valid_header(value: &str) -> bool {
    value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

#[cfg(test)]
#[path = "_tests_/config_tests.rs"]
mod config_tests;
