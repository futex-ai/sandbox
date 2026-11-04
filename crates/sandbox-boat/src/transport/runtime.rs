//! Bounded runtime HTTPS requests and long-lived attach bodies.

use std::{pin::Pin, time::Duration};

use async_trait::async_trait;
use futures_core::Stream;
use futures_util::{StreamExt, stream};

use crate::error::{Error, Result};

use super::{http::ReqwestBoatHttpTransport, runtime_route::RuntimeRoute};

/// Bounded bytes from an authenticated attach response.
pub type BoatByteStream = Pin<Box<dyn Stream<Item = Result<Vec<u8>>> + Send>>;

/// One runtime request. Its payload is deliberately absent from diagnostics.
pub struct BoatRuntimeRequest {
    /// Validated runtime route and operation.
    pub route: RuntimeRoute,
    /// Exact JSON request, when present.
    pub body: Option<Vec<u8>>,
    /// Remaining absolute operation budget.
    pub timeout: Duration,
    /// Maximum complete response bytes.
    pub response_limit: usize,
}

/// Credentialed runtime transport sharing the control origin and scope policy.
#[unimock::unimock(api = BoatRuntimeTransportMock)]
#[async_trait]
pub trait BoatRuntimeTransport: Send + Sync {
    /// Sends one request without retrying it.
    async fn request(&self, request: BoatRuntimeRequest) -> Result<Vec<u8>>;
    /// Opens a bounded-frame attach stream without a 60-second control cap.
    async fn attach(&self, request: BoatRuntimeRequest) -> Result<BoatByteStream>;
}

impl ReqwestBoatHttpTransport {
    async fn runtime_response(&self, request: BoatRuntimeRequest) -> Result<reqwest::Response> {
        if request.timeout.is_zero() || request.timeout > Duration::from_secs(3610) {
            return Err(Error::InvalidRequest);
        }
        let (method, path) = request.route.path()?;
        let url = format!("{}{path}", self.config.api_base());
        let mut builder = self
            .client
            .request(method, url)
            .timeout(request.timeout)
            .bearer_auth(self.config.api_key())
            .header(
                "User-Agent",
                concat!("sandbox-boat/", env!("CARGO_PKG_VERSION")),
            );
        if let Some(org) = self.config.organization() {
            builder = builder.header("X-Boat-Org", org);
        }
        if let Some(body) = request.body {
            builder = builder
                .header("Content-Type", "application/json")
                .body(body);
        }
        let response = match builder.send().await {
            Ok(response) => response,
            Err(_) => return Err(Error::DeliveryAmbiguous),
        };
        match response.status().as_u16() {
            200 => Ok(response),
            404 => Err(Error::NotFound),
            401 | 403 => Err(Error::Unauthorized),
            _ => Err(Error::Unavailable),
        }
    }
}

#[async_trait]
impl BoatRuntimeTransport for ReqwestBoatHttpTransport {
    async fn request(&self, request: BoatRuntimeRequest) -> Result<Vec<u8>> {
        let maximum = request.response_limit;
        if maximum == 0 || maximum > 6 * 1024 * 1024 {
            return Err(Error::InvalidRequest);
        }
        let mut response = self.runtime_response(request).await?;
        if response
            .content_length()
            .is_some_and(|n| n > maximum as u64)
        {
            return Err(Error::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        loop {
            let chunk = match response.chunk().await {
                Ok(chunk) => chunk,
                Err(_) => return Err(Error::Unavailable),
            };
            let Some(chunk) = chunk else {
                return Ok(bytes);
            };
            if chunk.len() > maximum.saturating_sub(bytes.len()) {
                return Err(Error::ResponseTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
    }

    async fn attach(&self, request: BoatRuntimeRequest) -> Result<BoatByteStream> {
        if !matches!(request.route, RuntimeRoute::Attach { .. }) {
            return Err(Error::InvalidRequest);
        }
        let response = self.runtime_response(request).await?;
        let bytes = response.bytes_stream().map(|item| match item {
            Ok(bytes) => Ok(bytes.to_vec()),
            Err(_) => Err(Error::Unavailable),
        });
        Ok(Box::pin(stream::unfold(
            Box::pin(bytes),
            |mut bytes| async move { bytes.next().await.map(|item| (item, bytes)) },
        )))
    }
}
