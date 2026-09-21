//! Credential-free interface and E2B construction smoke test.

use std::collections::HashMap;

use sandbox_e2b::{E2bAdapterConfig, E2bProfile, E2bSandboxBackend};
use sandbox_interface::{SandboxId, ScreenViewportSize};

use crate::error::{Error, Result};

pub(crate) fn run() -> Result<()> {
    construct("https://api.e2b.app", "placeholder-api-key")
}

fn construct(api_base: &str, api_key: &str) -> Result<()> {
    let profiles = HashMap::from([(
        "general".to_owned(),
        E2bProfile {
            template: "placeholder-template".to_owned(),
            allow_public_egress: true,
            denied_destinations: vec!["169.254.169.254/32".to_owned()],
        },
    )]);
    let config = E2bAdapterConfig::new("e2b", api_base, api_key, profiles, 600)
        .map_err(|source| Error::SmokeConfig { source })?;
    let _backend = E2bSandboxBackend::new(config)
        .map_err(|source| Error::SmokeBackend { source })?
        .into_backend();
    let _sandbox_id = SandboxId::new();
    let _viewport =
        ScreenViewportSize::new(1280, 720).map_err(|source| Error::SmokeBackend { source })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::construct;

    #[test]
    fn accepts_placeholder_credentials_without_provider_dispatch() {
        construct("https://api.e2b.app", "placeholder-api-key").unwrap();
    }

    #[test]
    fn rejects_invalid_configuration_before_backend_construction() {
        assert!(construct("http://api.e2b.app", "placeholder-api-key").is_err());
    }
}
