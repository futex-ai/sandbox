//! Production HTTPS client with exact origin, no redirects, and bounded bodies.

use async_trait::async_trait;

use crate::{
    config::BoatAdapterConfig,
    error::{Error, Result},
};

use super::{
    request::{BoatHttpMethod, BoatHttpRequest, BoatRoute},
    settings::BoatHttpSettings,
};

/// HTTP response data needed by typed Boat control decoders.
#[derive(Clone)]
pub struct BoatHttpResponse {
    /// Provider HTTP status.
    pub status: u16,
    /// Exact response bytes within the configured bound.
    pub body: Vec<u8>,
    /// Exact retry timing header, if supplied.
    pub retry_after: Option<String>,
}

/// Injectable transport used by Boat control operations.
#[unimock::unimock(api = BoatHttpTransportMock)]
#[async_trait]
pub trait BoatHttpTransport: Send + Sync {
    /// Sends one request. It must never replay a mutating request.
    async fn send(&self, request: BoatHttpRequest) -> Result<BoatHttpResponse>;
}

/// Bounded reqwest implementation with account credentials outside the workload.
pub struct ReqwestBoatHttpTransport {
    pub(super) client: reqwest::Client,
    pub(super) config: BoatAdapterConfig,
    settings: BoatHttpSettings,
}

impl ReqwestBoatHttpTransport {
    /// Constructs a client using system trust roots and strict default budgets.
    pub fn new(config: BoatAdapterConfig) -> Result<Self> {
        Self::with_settings(config, BoatHttpSettings::default(), None)
    }

    /// Constructs a strict HTTPS client with optional additional PEM trust roots.
    ///
    /// Extra roots permit private HTTPS deployments and local TLS fixtures.
    /// Certificate and host verification remain active.
    pub fn with_settings(
        config: BoatAdapterConfig,
        settings: BoatHttpSettings,
        extra_root_pem: Option<&[u8]>,
    ) -> Result<Self> {
        settings.validate()?;
        let mut builder = reqwest::Client::builder()
            .https_only(true)
            .connect_timeout(settings.connect_timeout)
            .read_timeout(settings.read_timeout)
            .timeout(settings.request_timeout)
            .redirect(reqwest::redirect::Policy::none());
        if let Some(pem) = extra_root_pem {
            let certificates = match reqwest::Certificate::from_pem_bundle(pem) {
                Ok(certificates) if !certificates.is_empty() => certificates,
                Ok(_) => return Err(Error::InvalidRequest),
                Err(_) => return Err(Error::InvalidRequest),
            };
            for certificate in certificates {
                builder = builder.add_root_certificate(certificate);
            }
        }
        let client = match builder.build() {
            Ok(client) => client,
            Err(source) => return Err(Error::internal_with(source, "build Boat HTTPS client")),
        };
        Ok(Self {
            client,
            config,
            settings,
        })
    }
}

#[async_trait]
impl BoatHttpTransport for ReqwestBoatHttpTransport {
    async fn send(&self, request: BoatHttpRequest) -> Result<BoatHttpResponse> {
        let path = request.route.path(request.method)?;
        let url = format!("{}{path}", self.config.api_base());
        let mut builder = match request.method {
            BoatHttpMethod::Get => self.client.get(url),
            BoatHttpMethod::Post => self.client.post(url),
            BoatHttpMethod::Delete => self.client.delete(url),
        }
        .bearer_auth(self.config.api_key())
        .header(
            "User-Agent",
            concat!("sandbox-boat/", env!("CARGO_PKG_VERSION")),
        );
        if let Some(org) = self.config.organization() {
            builder = builder.header("X-Boat-Org", org);
        }
        if let Some(key) = request.idempotency_key {
            builder = builder.header("Idempotency-Key", key);
        }
        if request.method == BoatHttpMethod::Delete
            && let BoatRoute::Sandbox(id) = request.route
        {
            builder = builder.header("X-Ascii-Confirm-Delete", id);
        }
        if let Some(body) = request.body {
            builder = builder
                .header("Content-Type", "application/json")
                .body(body);
        }
        let ambiguous = request.method == BoatHttpMethod::Post;
        let mut response = match builder.send().await {
            Ok(response) => response,
            Err(_) => return Err(delivery_error(ambiguous)),
        };
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("Retry-After")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        if response
            .content_length()
            .is_some_and(|length| length > self.settings.max_response_bytes as u64)
        {
            return Err(limit_error(ambiguous));
        }
        let mut body = Vec::new();
        loop {
            let chunk = match response.chunk().await {
                Ok(chunk) => chunk,
                Err(_) => return Err(delivery_error(ambiguous)),
            };
            let Some(chunk) = chunk else {
                break;
            };
            if chunk.len() > self.settings.max_response_bytes.saturating_sub(body.len()) {
                return Err(limit_error(ambiguous));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(BoatHttpResponse {
            status,
            body,
            retry_after,
        })
    }
}

fn delivery_error(ambiguous: bool) -> Error {
    if ambiguous {
        Error::DeliveryAmbiguous
    } else {
        Error::Unavailable
    }
}

fn limit_error(ambiguous: bool) -> Error {
    if ambiguous {
        Error::DeliveryAmbiguous
    } else {
        Error::ResponseTooLarge
    }
}
