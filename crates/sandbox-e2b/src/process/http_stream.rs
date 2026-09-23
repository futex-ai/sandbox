//! Reqwest-backed Connect server-streaming request support.

use std::time::Duration;

use futures_util::StreamExt;

use crate::error::Result;

use super::{
    framing::encode_frame,
    http::{ByteStream, ReqwestConnectHttpTransport, map_status},
    http_error::request_error,
    types::ProcessConnection,
};

pub(super) async fn open(
    transport: &ReqwestConnectHttpTransport,
    connection: ProcessConnection,
    method: String,
    request_json: Vec<u8>,
    request_timeout: Option<Duration>,
) -> Result<ByteStream> {
    let ambiguous = method == "Start";
    let response = match request(
        transport,
        &connection,
        &method,
        &request_json,
        request_timeout,
    )?
    .send()
    .await
    {
        Ok(response) => response,
        Err(source) => return Err(request_error(source, ambiguous, "open Connect stream")),
    };
    map_status(response.status().as_u16(), ambiguous)?;
    let response_stream = response.bytes_stream().map(move |item| match item {
        Ok(bytes) => Ok(bytes),
        Err(source) => Err(request_error(source, ambiguous, "read Connect stream")),
    });
    Ok(Box::pin(response_stream))
}

pub(super) fn request(
    transport: &ReqwestConnectHttpTransport,
    connection: &ProcessConnection,
    method: &str,
    request_json: &[u8],
    request_timeout: Option<Duration>,
) -> Result<reqwest::RequestBuilder> {
    let request = transport
        .request(connection, method, "application/connect+json")?
        .body(server_streaming_request_body(request_json)?);
    Ok(match request_timeout {
        Some(timeout) => request.timeout(timeout),
        None => request,
    })
}

pub(super) fn server_streaming_request_body(request_json: &[u8]) -> Result<Vec<u8>> {
    encode_frame(request_json)
}
