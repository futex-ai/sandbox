//! Shared bounded request helpers for the E2B control client.

use std::sync::Arc;

use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

use super::{
    client::ReqwestE2bControlApi,
    helpers::ensure_status,
    http::{HttpRequest, HttpResponse, Method, ReqwestE2bHttpTransport},
    types::SandboxDetailBody,
};

impl ReqwestE2bControlApi {
    /// Builds a typed control client with a bounded Reqwest transport.
    pub fn new(
        api_base: impl Into<String>,
        api_key: impl Into<String>,
        sandbox_domain: impl Into<String>,
        idle_timeout_seconds: u32,
    ) -> Result<Self> {
        let transport = ReqwestE2bHttpTransport::new(api_base.into(), api_key.into())?;
        Ok(Self {
            transport: Arc::new(transport),
            sandbox_domain: sandbox_domain.into(),
            idle_timeout_seconds,
        })
    }

    async fn request(
        &self,
        method: Method,
        path_and_query: String,
        body: Option<Vec<u8>>,
        ambiguous: bool,
    ) -> Result<HttpResponse> {
        self.transport
            .send(HttpRequest {
                method,
                path_and_query,
                body,
                ambiguous_on_failure: ambiguous,
            })
            .await
    }

    pub(super) async fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: String,
        body: Option<Vec<u8>>,
        accepted: &[u16],
        ambiguous: bool,
    ) -> Result<T> {
        let (value, _) = self
            .json_page(method, path, body, accepted, ambiguous)
            .await?;
        Ok(value)
    }

    pub(super) async fn json_page<T: DeserializeOwned>(
        &self,
        method: Method,
        path: String,
        body: Option<Vec<u8>>,
        accepted: &[u16],
        ambiguous: bool,
    ) -> Result<(T, Option<String>)> {
        let response = self.request(method, path, body, ambiguous).await?;
        ensure_status(response.status, accepted, ambiguous)?;
        match serde_json::from_slice(&response.body) {
            Ok(value) => Ok((value, response.next_token)),
            Err(_) if ambiguous => Err(Error::DeliveryAmbiguous),
            Err(source) => Err(Error::internal_with(source, "decode E2B control response")),
        }
    }

    pub(super) async fn empty(
        &self,
        method: Method,
        path: String,
        body: Option<Vec<u8>>,
        accepted: &[u16],
    ) -> Result<()> {
        let response = self.request(method, path, body, false).await?;
        ensure_status(response.status, accepted, false)
    }

    pub(super) async fn sandbox_detail(&self, sandbox_id: &str) -> Result<SandboxDetailBody> {
        self.json(
            Method::Get,
            format!("/sandboxes/{}", super::helpers::path_segment(sandbox_id)),
            None,
            &[200],
            false,
        )
        .await
    }
}
