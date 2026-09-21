//! Minimal bounded HTTP transport for the E2B control API.

use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;

use crate::error::{Error, Result};
use crate::response_body::{ResponseByteStream, collect_bounded};

const MAX_CONTROL_RESPONSE_BYTES: usize = 1024 * 1024;
const CONTROL_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CONTROL_READ_TIMEOUT: Duration = Duration::from_secs(30);
const CONTROL_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Method {
    Get,
    Post,
    Delete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HttpRequest {
    pub(super) method: Method,
    pub(super) path_and_query: String,
    pub(super) body: Option<Vec<u8>>,
    pub(super) ambiguous_on_failure: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HttpResponse {
    pub(super) status: u16,
    pub(super) body: Vec<u8>,
    pub(super) next_token: Option<String>,
}

#[unimock::unimock(api = [send])]
#[async_trait]
pub(crate) trait E2bHttpTransport: Send + Sync {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse>;
}

pub(super) struct ReqwestE2bHttpTransport {
    client: reqwest::Client,
    api_base: String,
    api_key: String,
}

impl ReqwestE2bHttpTransport {
    pub(super) fn new(api_base: String, api_key: String) -> Result<Self> {
        Self::new_with_timeouts(
            api_base,
            api_key,
            CONTROL_CONNECT_TIMEOUT,
            CONTROL_READ_TIMEOUT,
            CONTROL_REQUEST_TIMEOUT,
        )
    }

    fn new_with_timeouts(
        api_base: String,
        api_key: String,
        connect_timeout: Duration,
        read_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self> {
        let client = match reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .read_timeout(read_timeout)
            .timeout(request_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
        {
            Ok(client) => client,
            Err(source) => return Err(Error::internal_with(source, "build E2B HTTP client")),
        };
        Ok(Self {
            client,
            api_base: api_base.trim_end_matches('/').to_owned(),
            api_key,
        })
    }
}

#[async_trait]
impl E2bHttpTransport for ReqwestE2bHttpTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse> {
        let HttpRequest {
            method,
            path_and_query,
            body,
            ambiguous_on_failure,
        } = request;
        let url = format!("{}{}", self.api_base, path_and_query);
        let mut builder = match method {
            Method::Get => self.client.get(url),
            Method::Post => self.client.post(url),
            Method::Delete => self.client.delete(url),
        }
        .header("X-API-KEY", &self.api_key)
        .header(
            "User-Agent",
            concat!("sandbox-e2b/", env!("CARGO_PKG_VERSION")),
        );
        if let Some(body) = body {
            builder = builder
                .header("Content-Type", "application/json")
                .body(body);
        }
        let response = match builder.send().await {
            Ok(response) => response,
            Err(source) => {
                return Err(request_error(
                    source,
                    ambiguous_on_failure,
                    "send E2B control request",
                ));
            }
        };
        let status = response.status().as_u16();
        if !(200..=299).contains(&status) {
            return Ok(HttpResponse {
                status,
                body: Vec::new(),
                next_token: None,
            });
        }
        let next_token = match response.headers().get("x-next-token") {
            Some(value) => match value.to_str() {
                Ok(value) if value.trim().is_empty() => None,
                Ok(value) => Some(value.to_owned()),
                Err(_) => return Err(Error::InvalidPagination),
            },
            None => None,
        };
        if response
            .content_length()
            .is_some_and(|length| length > MAX_CONTROL_RESPONSE_BYTES as u64)
        {
            return Err(response_limit_error(status, ambiguous_on_failure));
        }
        let response_stream: ResponseByteStream =
            Box::pin(response.bytes_stream().map(move |item| match item {
                Ok(bytes) => Ok(bytes),
                Err(source) => Err(request_error(
                    source,
                    ambiguous_on_failure,
                    "read E2B control response",
                )),
            }));
        let body = match collect_bounded(response_stream, MAX_CONTROL_RESPONSE_BYTES).await {
            Err(Error::ResponseTooLarge) => {
                return Err(response_limit_error(status, ambiguous_on_failure));
            }
            result => result?,
        };
        Ok(HttpResponse {
            status,
            body,
            next_token,
        })
    }
}

fn response_limit_error(status: u16, ambiguous: bool) -> Error {
    if ambiguous && (200..=299).contains(&status) {
        return Error::DeliveryAmbiguous;
    }
    Error::ResponseTooLarge
}

fn request_error(source: reqwest::Error, ambiguous: bool, context: &'static str) -> Error {
    if ambiguous {
        return Error::DeliveryAmbiguous;
    }
    if source.is_timeout()
        || source.is_connect()
        || source.is_body()
        || source.is_decode()
        || source.is_request()
    {
        return Error::Unavailable;
    }
    Error::internal_with(source, context)
}

#[cfg(test)]
#[path = "_tests_/control_http_tests.rs"]
mod control_http_tests;

#[cfg(test)]
#[path = "_tests_/control_http_error_tests.rs"]
mod control_http_error_tests;
