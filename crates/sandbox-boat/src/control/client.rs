//! Boat control client composed over an injected HTTP transport.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    error::{Error, Result},
    transport::{
        http::BoatHttpTransport,
        request::{BoatHttpMethod, BoatHttpRequest, BoatRoute},
    },
};

use super::{
    acknowledgment,
    api::BoatControlApi,
    deletion, payload, sandboxes, snapshots,
    types::{
        BoatCreateSandbox, BoatLabels, BoatLimits, BoatSandbox, BoatSaveSnapshot, BoatSavedSnapshot,
    },
};

/// Typed control decoder with a replaceable credentialed transport.
pub struct ReqwestBoatControlApi {
    pub(super) transport: Arc<dyn BoatHttpTransport>,
}

impl ReqwestBoatControlApi {
    /// Constructs the control client over one validated transport.
    #[must_use]
    pub fn with_transport(transport: Arc<dyn BoatHttpTransport>) -> Self {
        Self { transport }
    }
}

#[async_trait]
impl BoatControlApi for ReqwestBoatControlApi {
    async fn create_sandbox(&self, request: BoatCreateSandbox) -> Result<BoatSandbox> {
        sandboxes::create(self, request).await
    }

    async fn list_sandboxes(&self, labels: BoatLabels) -> Result<Vec<BoatSandbox>> {
        sandboxes::list(self, labels).await
    }

    async fn get_sandbox(&self, id: &str) -> Result<BoatSandbox> {
        sandboxes::get(self, id).await
    }

    async fn delete_sandbox(&self, id: &str) -> Result<()> {
        deletion::delete(self, id).await
    }

    async fn get_snapshot(&self, name: &str) -> Result<BoatSavedSnapshot> {
        snapshots::get(self, name).await
    }

    async fn save_snapshot(&self, request: BoatSaveSnapshot) -> Result<BoatSavedSnapshot> {
        snapshots::save(self, request).await
    }

    async fn delete_snapshot(&self, name: &str) -> Result<()> {
        snapshots::delete(self, name).await
    }

    async fn limits(&self) -> Result<BoatLimits> {
        let response = self
            .transport
            .send(BoatHttpRequest {
                method: BoatHttpMethod::Get,
                route: BoatRoute::Limits,
                body: None,
                idempotency_key: None,
            })
            .await?;
        if response.status != 200 {
            return Err(acknowledgment::read_status(response.status));
        }
        let limits: LimitsEnvelope = payload::decode(&response.body)?;
        if !limits.ok || limits.kind != "limits.info" {
            return Err(Error::InvalidResponse);
        }
        Ok(limits.limits)
    }
}

#[derive(Deserialize)]
struct LimitsEnvelope {
    ok: bool,
    #[serde(rename = "type")]
    kind: String,
    #[serde(flatten)]
    limits: BoatLimits,
}
