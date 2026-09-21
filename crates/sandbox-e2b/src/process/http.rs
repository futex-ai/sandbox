//! Bounded Reqwest transport for Connect JSON unary and streaming calls.

use std::{pin::Pin, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use futures_core::Stream;
use futures_util::StreamExt;

use crate::error::{Error, Result};

use super::{framing::encode_frame, types::ProcessConnection};

pub(crate) type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>;

#[unimock::unimock(api = [stream, unary, download, upload])]
#[async_trait]
pub(crate) trait ConnectHttpTransport: Send + Sync {
    async fn stream(
        &self,
        connection: ProcessConnection,
        method: String,
        request_json: Vec<u8>,
    ) -> Result<ByteStream>;
    async fn unary(
        &self,
        connection: ProcessConnection,
        method: String,
        request_json: Vec<u8>,
        ambiguous: bool,
    ) -> Result<Vec<u8>>;
    async fn download(
        &self,
        connection: ProcessConnection,
        path: String,
        offset: u64,
        max_bytes: usize,
    ) -> Result<Vec<u8>>;
    async fn upload(
        &self,
        connection: ProcessConnection,
        path: String,
        bytes: Vec<u8>,
    ) -> Result<()>;
}

pub(super) struct ReqwestConnectHttpTransport {
    client: reqwest::Client,
}

impl ReqwestConnectHttpTransport {
    pub(super) fn new() -> Result<Self> {
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(310))
            .build()
        {
            Ok(client) => client,
            Err(source) => return Err(Error::internal_with(source, "build Connect HTTP client")),
        };
        Ok(Self { client })
    }

    fn request(
        &self,
        connection: &ProcessConnection,
        method: &str,
        content_type: &str,
    ) -> reqwest::RequestBuilder {
        self.client
            .post(format!(
                "https://49983-{}.{}/process.Process/{method}",
                connection.sandbox_id, connection.sandbox_domain
            ))
            .header("Content-Type", content_type)
            .header("Connect-Protocol-Version", "1")
            .header("X-Access-Token", &connection.access_token)
            .header(
                "User-Agent",
                concat!("sandbox-e2b/", env!("CARGO_PKG_VERSION")),
            )
    }

    fn file_request(
        &self,
        connection: &ProcessConnection,
        method: reqwest::Method,
        path: &str,
    ) -> reqwest::RequestBuilder {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("path", path)
            .finish();
        self.client
            .request(
                method,
                format!(
                    "https://49983-{}.{}/files?{query}",
                    connection.sandbox_id, connection.sandbox_domain,
                ),
            )
            .header("X-Access-Token", &connection.access_token)
            .header(
                "User-Agent",
                concat!("sandbox-e2b/", env!("CARGO_PKG_VERSION")),
            )
    }
}

#[async_trait]
impl ConnectHttpTransport for ReqwestConnectHttpTransport {
    async fn stream(
        &self,
        connection: ProcessConnection,
        method: String,
        request_json: Vec<u8>,
    ) -> Result<ByteStream> {
        let request_body = server_streaming_request_body(&request_json)?;
        let response = match self
            .request(&connection, &method, "application/connect+json")
            .body(request_body)
            .send()
            .await
        {
            Ok(response) => response,
            Err(source) => return Err(Error::internal_with(source, "open Connect stream")),
        };
        map_status(response.status().as_u16(), false)?;
        let response_stream = response.bytes_stream().map(|item| match item {
            Ok(bytes) => Ok(bytes),
            Err(source) => Err(Error::internal_with(source, "read Connect stream")),
        });
        Ok(Box::pin(response_stream))
    }

    async fn unary(
        &self,
        connection: ProcessConnection,
        method: String,
        request_json: Vec<u8>,
        ambiguous: bool,
    ) -> Result<Vec<u8>> {
        let response = match self
            .request(&connection, &method, "application/json")
            .body(request_json)
            .send()
            .await
        {
            Ok(response) => response,
            Err(source) if ambiguous => {
                let _ = source;
                return Err(Error::DeliveryAmbiguous);
            }
            Err(source) => return Err(Error::internal_with(source, "send Connect unary request")),
        };
        map_status(response.status().as_u16(), ambiguous)?;
        match response.bytes().await {
            Ok(bytes) if bytes.len() <= 1024 * 1024 => Ok(bytes.to_vec()),
            Ok(_) => Err(Error::ResponseTooLarge),
            Err(source) if ambiguous => {
                let _ = source;
                Err(Error::DeliveryAmbiguous)
            }
            Err(source) => Err(Error::internal_with(source, "read Connect unary response")),
        }
    }

    async fn download(
        &self,
        connection: ProcessConnection,
        path: String,
        offset: u64,
        max_bytes: usize,
    ) -> Result<Vec<u8>> {
        let maximum = match u64::try_from(max_bytes) {
            Ok(maximum) => maximum,
            Err(source) => return Err(Error::internal_with(source, "convert file download limit")),
        };
        let Some(end) = offset.checked_add(maximum.saturating_sub(1)) else {
            return Err(Error::InvalidRequest);
        };
        let response = match self
            .file_request(&connection, reqwest::Method::GET, &path)
            .header("Range", format!("bytes={offset}-{end}"))
            .send()
            .await
        {
            Ok(response) => response,
            Err(source) => return Err(Error::internal_with(source, "download envd file")),
        };
        map_status(response.status().as_u16(), false)?;
        match response.bytes().await {
            Ok(bytes) if bytes.len() <= max_bytes => Ok(bytes.to_vec()),
            Ok(_) => Err(Error::ResponseTooLarge),
            Err(source) => Err(Error::internal_with(source, "read envd file response")),
        }
    }

    async fn upload(
        &self,
        connection: ProcessConnection,
        path: String,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let response = match self
            .file_request(&connection, reqwest::Method::POST, &path)
            .header("Content-Type", "application/octet-stream")
            .body(bytes)
            .send()
            .await
        {
            Ok(response) => response,
            Err(source) => return Err(Error::internal_with(source, "upload envd file")),
        };
        map_status(response.status().as_u16(), false)
    }
}

fn server_streaming_request_body(request_json: &[u8]) -> Result<Vec<u8>> {
    encode_frame(request_json)
}

fn map_status(status: u16, ambiguous: bool) -> Result<()> {
    match status {
        200..=299 => Ok(()),
        400 => Err(Error::InvalidRequest),
        401 | 403 => Err(Error::Unauthorized),
        404 => Err(Error::NotFound),
        408 | 429 | 500..=599 if ambiguous => Err(Error::DeliveryAmbiguous),
        408 | 429 | 500..=599 => Err(Error::Unavailable),
        _ => Err(Error::InvalidRequest),
    }
}

#[cfg(test)]
#[path = "_tests_/process_http_tests.rs"]
mod process_http_tests;
