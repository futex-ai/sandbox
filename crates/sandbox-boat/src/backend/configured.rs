//! Boat backend composition over an injected typed control client.

use std::sync::Arc;

use sandbox_interface::Result;

use crate::{
    config::BoatAdapterConfig,
    control::{api::BoatControlApi, client::ReqwestBoatControlApi},
    runtime::{
        api::BoatRuntimeApi,
        client::HttpsBoatRuntimeApi,
        configured::{BoatRuntime, BoatTrustedRuntime},
    },
    transport::http::ReqwestBoatHttpTransport,
};

use super::mapping;

/// Boat lifecycle, saved-snapshot, and account-capacity implementation.
pub struct BoatSandboxBackend {
    pub(super) config: BoatAdapterConfig,
    pub(super) control: Arc<dyn BoatControlApi>,
    pub(crate) runtime: Arc<dyn BoatRuntime>,
}

impl BoatSandboxBackend {
    /// Constructs the production HTTPS control transport without provider calls.
    pub fn new(config: BoatAdapterConfig) -> Result<Self> {
        let transport = mapping::control_result(
            ReqwestBoatHttpTransport::new(config.clone()),
            config.backend_id(),
            None,
        )?;
        let transport = Arc::new(transport);
        let control = Arc::new(ReqwestBoatControlApi::with_transport(transport.clone()));
        let api: Arc<dyn BoatRuntimeApi> = Arc::new(HttpsBoatRuntimeApi::new(transport));
        let runtime = Arc::new(BoatTrustedRuntime::new(config.clone(), api));
        Ok(Self {
            config,
            control,
            runtime,
        })
    }

    /// Constructs a backend with explicit control and trusted runtime collaborators.
    #[must_use]
    pub fn with_clients(
        config: BoatAdapterConfig,
        control: Arc<dyn BoatControlApi>,
        runtime: Arc<dyn BoatRuntime>,
    ) -> Self {
        Self {
            config,
            control,
            runtime,
        }
    }
}
