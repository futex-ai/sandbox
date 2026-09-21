//! Validated adapter configuration and deployment-owned profiles.

use std::{collections::HashMap, fmt, net::IpAddr, path::Component, path::Path};

use sandbox_interface::{SANDBOX_PROFILE_MAX_ITEMS, valid_sandbox_profile_name};
use url::Url;

use crate::error::{Error, Result};

const DEFAULT_SANDBOX_DOMAIN: &str = "e2b.app";
const DEFAULT_METADATA_PREFIX: &str = "sandbox";
const DEFAULT_TERMINAL_TAG_PREFIX: &str = "sandbox-terminal-";
const DEFAULT_SCREEN_HELPER_PATH: &str = "/usr/local/bin/sandbox-screen";
const CONVENTION_MAX_BYTES: usize = 255;

/// One deployment-owned E2B template and network posture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct E2bProfile {
    /// E2B template or snapshot template identifier.
    pub template: String,
    /// Whether ordinary public package and Git egress is allowed.
    pub allow_public_egress: bool,
    /// Additional deployment-owned egress destinations to deny.
    pub denied_destinations: Vec<String>,
}

/// Deployment-specific names embedded in E2B metadata and template processes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct E2bRuntimeConventions {
    metadata_prefix: String,
    terminal_tag_prefix: String,
    screen_helper_path: String,
}

impl E2bRuntimeConventions {
    /// Validates conventions used to find resources created by one deployment.
    pub fn new(
        metadata_prefix: impl Into<String>,
        terminal_tag_prefix: impl Into<String>,
        screen_helper_path: impl Into<String>,
    ) -> Result<Self> {
        let conventions = Self {
            metadata_prefix: metadata_prefix.into(),
            terminal_tag_prefix: terminal_tag_prefix.into(),
            screen_helper_path: screen_helper_path.into(),
        };
        if !valid_metadata_prefix(&conventions.metadata_prefix)
            || !valid_terminal_tag_prefix(&conventions.terminal_tag_prefix)
            || !valid_absolute_path(&conventions.screen_helper_path)
        {
            return Err(Error::InvalidRequest);
        }
        Ok(conventions)
    }

    /// Prefix used for E2B resource-correlation metadata keys.
    #[must_use]
    pub fn metadata_prefix(&self) -> &str {
        &self.metadata_prefix
    }

    /// Prefix used for durable terminal process tags.
    #[must_use]
    pub fn terminal_tag_prefix(&self) -> &str {
        &self.terminal_tag_prefix
    }

    /// Absolute helper executable installed by compatible screen templates.
    #[must_use]
    pub fn screen_helper_path(&self) -> &str {
        &self.screen_helper_path
    }

    pub(crate) fn metadata_key(&self, suffix: &str) -> String {
        format!("{}_{suffix}", self.metadata_prefix)
    }
}

impl Default for E2bRuntimeConventions {
    fn default() -> Self {
        Self {
            metadata_prefix: DEFAULT_METADATA_PREFIX.to_owned(),
            terminal_tag_prefix: DEFAULT_TERMINAL_TAG_PREFIX.to_owned(),
            screen_helper_path: DEFAULT_SCREEN_HELPER_PATH.to_owned(),
        }
    }
}

/// Complete configuration for one E2B backend registration.
#[derive(Clone, Eq, PartialEq)]
pub struct E2bAdapterConfig {
    /// Stable backend ID persisted on consumer resources.
    pub backend_id: String,
    /// Injectable root E2B control API base URL.
    pub api_base: String,
    /// E2B sandbox routing domain.
    pub(crate) sandbox_domain: String,
    /// Worker-only E2B API key value.
    pub api_key: String,
    /// Logical profile-to-template mappings.
    pub profiles: HashMap<String, E2bProfile>,
    /// Sandbox idle timeout in seconds before E2B auto-pause.
    pub idle_timeout_seconds: u32,
    /// Names used to correlate resources and invoke template-owned helpers.
    pub runtime_conventions: E2bRuntimeConventions,
}

impl fmt::Debug for E2bAdapterConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("E2bAdapterConfig")
            .field("backend_id", &self.backend_id)
            .field("api_base", &self.api_base)
            .field("sandbox_domain", &self.sandbox_domain)
            .field("api_key", &"[redacted]")
            .field("profiles", &self.profiles)
            .field("idle_timeout_seconds", &self.idle_timeout_seconds)
            .field("runtime_conventions", &self.runtime_conventions)
            .finish()
    }
}

impl E2bAdapterConfig {
    /// Builds and validates adapter configuration.
    pub fn new(
        backend_id: impl Into<String>,
        api_base: impl Into<String>,
        api_key: impl Into<String>,
        profiles: HashMap<String, E2bProfile>,
        idle_timeout_seconds: u32,
    ) -> Result<Self> {
        let profiles = normalize_profiles(profiles).ok_or(Error::InvalidRequest)?;
        let config = Self {
            backend_id: backend_id.into(),
            api_base: api_base.into(),
            sandbox_domain: DEFAULT_SANDBOX_DOMAIN.to_owned(),
            api_key: api_key.into(),
            profiles,
            idle_timeout_seconds,
            runtime_conventions: E2bRuntimeConventions::default(),
        };
        if !nonempty_canonical(&config.backend_id)
            || !nonempty_canonical(&config.api_base)
            || !valid_api_base(&config.api_base)
            || !nonempty_canonical(&config.api_key)
            || config.profiles.is_empty()
            || config.idle_timeout_seconds == 0
            || config.profiles.values().any(|profile| {
                !nonempty_canonical(&profile.template) || profile.denied_destinations.is_empty()
            })
        {
            return Err(Error::InvalidRequest);
        }
        Ok(config)
    }

    /// Replaces neutral defaults with validated deployment compatibility names.
    #[must_use]
    pub fn with_runtime_conventions(mut self, conventions: E2bRuntimeConventions) -> Self {
        self.runtime_conventions = conventions;
        self
    }

    pub(crate) fn profile(&self, name: &str) -> Result<&E2bProfile> {
        self.profiles.get(name).ok_or(Error::InvalidRequest)
    }
}

fn valid_metadata_prefix(value: &str) -> bool {
    nonempty_canonical(value)
        && value.len() <= CONVENTION_MAX_BYTES
        && value.starts_with(|character: char| character.is_ascii_lowercase())
        && !value.ends_with('_')
        && !value.contains("__")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_terminal_tag_prefix(value: &str) -> bool {
    nonempty_canonical(value)
        && value.len() <= CONVENTION_MAX_BYTES
        && value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_absolute_path(value: &str) -> bool {
    nonempty_canonical(value)
        && value.len() <= CONVENTION_MAX_BYTES
        && value != "/"
        && !value.ends_with('/')
        && !value.contains("//")
        && !value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
        && Path::new(value).is_absolute()
        && Path::new(value)
            .components()
            .all(|component| !matches!(component, Component::CurDir | Component::ParentDir))
}

fn nonempty_canonical(value: &str) -> bool {
    !value.is_empty() && value.trim() == value
}

fn valid_api_base(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && url.has_host()
        && url.path() == "/"
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

fn normalize_profiles(
    mut profiles: HashMap<String, E2bProfile>,
) -> Option<HashMap<String, E2bProfile>> {
    if profiles.len() > SANDBOX_PROFILE_MAX_ITEMS
        || profiles
            .keys()
            .any(|name| !valid_sandbox_profile_name(name))
    {
        return None;
    }
    for profile in profiles.values_mut() {
        let mut destinations = Vec::with_capacity(profile.denied_destinations.len());
        for destination in &profile.denied_destinations {
            destinations.push(canonical_deny_destination(destination)?);
        }
        destinations.sort();
        destinations.dedup();
        profile.denied_destinations = destinations;
    }
    Some(profiles)
}

fn canonical_deny_destination(destination: &str) -> Option<String> {
    let destination = destination.trim();
    let (address, prefix) = match destination.split_once('/') {
        Some((address, prefix)) => (address, Some(prefix)),
        None => (destination, None),
    };
    let address = match address.parse::<IpAddr>() {
        Ok(address) => address,
        Err(_) => return None,
    };
    let Some(prefix) = prefix else {
        return Some(address.to_string());
    };
    let prefix = match prefix.parse::<u8>() {
        Ok(prefix) => prefix,
        Err(_) => return None,
    };
    let valid = match address {
        IpAddr::V4(_) => prefix <= 32,
        IpAddr::V6(_) => prefix <= 128,
    };
    if !valid {
        return None;
    }
    Some(format!("{address}/{prefix}"))
}

#[cfg(test)]
#[path = "_tests_/config_tests.rs"]
mod config_tests;
