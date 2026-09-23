//! Bounded Reqwest transport for Connect JSON unary and streaming calls.

use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use url::Url;

use crate::error::{Error, Result};
use crate::response_body::{ResponseByteStream, collect_bounded};

use super::{http_error::request_error, types::ProcessConnection};

pub(crate) type ByteStream = ResponseByteStream;

const MAX_UNARY_RESPONSE_BYTES: usize = 1024 * 1024;

#[unimock::unimock(api = [stream, stream_with_timeout, unary, download, upload])]
#[async_trait]
pub(crate) trait ConnectHttpTransport: Send + Sync {
    async fn stream(
        &self,
        connection: ProcessConnection,
        method: String,
        request_json: Vec<u8>,
    ) -> Result<ByteStream>;
    async fn stream_with_timeout(
        &self,
        connection: ProcessConnection,
        method: String,
        request_json: Vec<u8>,
        request_timeout: Duration,
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
            .redirect(reqwest::redirect::Policy::none())
            .build()
        {
            Ok(client) => client,
            Err(source) => return Err(Error::internal_with(source, "build Connect HTTP client")),
        };
        Ok(Self { client })
    }

    pub(super) fn request(
        &self,
        connection: &ProcessConnection,
        method: &str,
        content_type: &str,
    ) -> Result<reqwest::RequestBuilder> {
        let mut url = envd_base_url(connection)?;
        url.path_segments_mut()
            .map_err(|_| Error::InvalidRequest)?
            .extend(["process.Process", method]);
        let request = self
            .client
            .post(url)
            .header("Content-Type", content_type)
            .header("Connect-Protocol-Version", "1")
            .header("X-Access-Token", &connection.access_token)
            .header(
                "User-Agent",
                concat!("sandbox-e2b/", env!("CARGO_PKG_VERSION")),
            );
        Ok(match connection.user() {
            Some(user) => request.basic_auth(user, Some("")),
            None => request,
        })
    }

    fn file_request(
        &self,
        connection: &ProcessConnection,
        method: reqwest::Method,
        path: &str,
    ) -> Result<reqwest::RequestBuilder> {
        let mut url = envd_base_url(connection)?;
        url.set_path("/files");
        url.query_pairs_mut().append_pair("path", path);
        let request = self
            .client
            .request(method, url)
            .header("X-Access-Token", &connection.access_token)
            .header(
                "User-Agent",
                concat!("sandbox-e2b/", env!("CARGO_PKG_VERSION")),
            );
        Ok(match connection.user() {
            Some(user) => request.basic_auth(user, Some("")),
            None => request,
        })
    }
}

fn envd_base_url(connection: &ProcessConnection) -> Result<Url> {
    let expected_host = format!(
        "49983-{}.{}",
        connection.sandbox_id, connection.sandbox_domain
    );
    let url =
        Url::parse(&format!("https://{expected_host}/")).map_err(|_| Error::InvalidRequest)?;
    if url.scheme() != "https"
        || url.host_str() != Some(expected_host.as_str())
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::InvalidRequest);
    }
    Ok(url)
}

#[async_trait]
impl ConnectHttpTransport for ReqwestConnectHttpTransport {
    async fn stream(
        &self,
        connection: ProcessConnection,
        method: String,
        request_json: Vec<u8>,
    ) -> Result<ByteStream> {
        super::http_stream::open(self, connection, method, request_json, None).await
    }

    async fn stream_with_timeout(
        &self,
        connection: ProcessConnection,
        method: String,
        request_json: Vec<u8>,
        request_timeout: Duration,
    ) -> Result<ByteStream> {
        super::http_stream::open(
            self,
            connection,
            method,
            request_json,
            Some(request_timeout),
        )
        .await
    }

    async fn unary(
        &self,
        connection: ProcessConnection,
        method: String,
        request_json: Vec<u8>,
        ambiguous: bool,
    ) -> Result<Vec<u8>> {
        let response = match self
            .request(&connection, &method, "application/json")?
            .body(request_json)
            .send()
            .await
        {
            Ok(response) => response,
            Err(source) => {
                return Err(request_error(
                    source,
                    ambiguous,
                    "send Connect unary request",
                ));
            }
        };
        map_status(response.status().as_u16(), ambiguous)?;
        read_bounded_response(
            response,
            MAX_UNARY_RESPONSE_BYTES,
            ambiguous,
            "read Connect unary response",
        )
        .await
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
            .file_request(&connection, reqwest::Method::GET, &path)?
            .header("Range", format!("bytes={offset}-{end}"))
            .send()
            .await
        {
            Ok(response) => response,
            Err(source) => return Err(request_error(source, false, "download envd file")),
        };
        map_status(response.status().as_u16(), false)?;
        read_bounded_response(response, max_bytes, false, "read envd file response").await
    }

    async fn upload(
        &self,
        connection: ProcessConnection,
        path: String,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let response = match self
            .file_request(&connection, reqwest::Method::POST, &path)?
            .header("Content-Type", "application/octet-stream")
            .body(bytes)
            .send()
            .await
        {
            Ok(response) => response,
            Err(source) => return Err(request_error(source, true, "upload envd file")),
        };
        map_status(response.status().as_u16(), false)
    }
}

async fn read_bounded_response(
    response: reqwest::Response,
    maximum: usize,
    ambiguous: bool,
    context: &'static str,
) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        return Err(if ambiguous {
            Error::DeliveryAmbiguous
        } else {
            Error::ResponseTooLarge
        });
    }
    let response_stream: ResponseByteStream =
        Box::pin(response.bytes_stream().map(move |item| match item {
            Ok(bytes) => Ok(bytes),
            Err(source) => Err(request_error(source, ambiguous, context)),
        }));
    match collect_bounded(response_stream, maximum).await {
        Err(Error::ResponseTooLarge) if ambiguous => Err(Error::DeliveryAmbiguous),
        result => result,
    }
}

pub(super) fn map_status(status: u16, ambiguous: bool) -> Result<()> {
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
