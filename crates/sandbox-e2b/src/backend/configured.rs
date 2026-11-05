//! E2B backend construction and provider-neutral trait dispatch.

use std::sync::Arc;

use sandbox_interface::Result;

use crate::{
    config::E2bAdapterConfig,
    control::{DynE2bControlApi, ReqwestE2bControlApi},
    process::{ConnectProcessTransport, DynProcessTransport},
};

use super::mapping;

/// E2B implementation of the provider-neutral sandbox capabilities.
pub struct E2bSandboxBackend {
    pub(super) config: E2bAdapterConfig,
    pub(super) control: DynE2bControlApi,
    pub(super) processes: DynProcessTransport,
}

impl E2bSandboxBackend {
    /// Builds a production E2B backend from validated adapter configuration.
    pub fn new(config: E2bAdapterConfig) -> Result<Self> {
        let control = mapping::control_result(
            ReqwestE2bControlApi::new(
                config.api_base().to_owned(),
                config.api_key().to_owned(),
                config.sandbox_domain().to_owned(),
                config.idle_timeout_seconds(),
            )
            .map(|control| {
                control.with_lifetime_metadata_key(
                    config.runtime_conventions().metadata_key("lifetime"),
                )
            }),
            config.backend_id(),
            None,
        )?;
        let processes = ConnectProcessTransport::new(config.backend_id().to_owned())?;
        Ok(Self {
            config,
            control: Arc::new(control),
            processes: Arc::new(processes),
        })
    }

    /// Builds a backend with injected control and process transports.
    #[must_use]
    pub fn with_transports(
        config: E2bAdapterConfig,
        control: DynE2bControlApi,
        processes: DynProcessTransport,
    ) -> Self {
        Self {
            config,
            control,
            processes,
        }
    }
}
