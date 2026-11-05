//! Credential-free interface, E2B, and Boat construction smoke test.

use std::{collections::HashMap, sync::Arc};

use sandbox_boat::{BoatAdapterConfig, BoatProfile, BoatSandboxBackend};
use sandbox_e2b::{E2bAdapterConfig, E2bProfile, E2bSandboxBackend};
use sandbox_interface::{
    SandboxAccountCapacity, SandboxFiles, SandboxId, SandboxImagePreparation, SandboxLifecycle,
    SandboxPortIngress, SandboxProcesses, SandboxResumableLifecycle, SandboxScreens,
    SandboxSnapshots, SandboxTerminals, ScreenViewportSize,
};

use crate::error::{Error, Result};

pub(crate) fn run() -> Result<()> {
    construct("https://api.e2b.app", "placeholder-api-key")?;
    construct_boat()
}

fn construct_boat() -> Result<()> {
    let config = match BoatAdapterConfig::new(
        "boat",
        "https://boat.dev",
        "placeholder-api-key",
        [(
            "general".to_owned(),
            BoatProfile {
                provider_size: "default".to_owned(),
                allow_public_egress: true,
                denied_destinations: Vec::new(),
            },
        )]
        .into(),
    ) {
        Ok(config) => config,
        Err(source) => return Err(Error::SmokeBoatConfig { source }),
    };
    let backend = match BoatSandboxBackend::new(config) {
        Ok(backend) => Arc::new(backend),
        Err(source) => return Err(Error::SmokeBackend { source }),
    };
    let _lifecycle: Arc<dyn SandboxLifecycle> = backend.clone();
    let _snapshots: Arc<dyn SandboxSnapshots> = backend.clone();
    let _processes: Arc<dyn SandboxProcesses> = backend.clone();
    let _files: Arc<dyn SandboxFiles> = backend.clone();
    let _capacity: Arc<dyn SandboxAccountCapacity> = backend;
    Ok(())
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
    let backend =
        Arc::new(E2bSandboxBackend::new(config).map_err(|source| Error::SmokeBackend { source })?);
    let _lifecycle: Arc<dyn SandboxLifecycle> = backend.clone();
    let _resumable: Arc<dyn SandboxResumableLifecycle> = backend.clone();
    let _snapshots: Arc<dyn SandboxSnapshots> = backend.clone();
    let _images: Arc<dyn SandboxImagePreparation> = backend.clone();
    let _processes: Arc<dyn SandboxProcesses> = backend.clone();
    let _files: Arc<dyn SandboxFiles> = backend.clone();
    let _terminals: Arc<dyn SandboxTerminals> = backend.clone();
    let _ingress: Arc<dyn SandboxPortIngress> = backend.clone();
    let _screens: Arc<dyn SandboxScreens> = backend;
    let _sandbox_id = SandboxId::new();
    let _viewport =
        ScreenViewportSize::new(1280, 720).map_err(|source| Error::SmokeBackend { source })?;
    Ok(())
}

#[cfg(test)]
#[path = "_tests_/smoke_tests.rs"]
mod smoke_tests;
