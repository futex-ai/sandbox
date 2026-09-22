//! Shared streaming collection for bounded HTTP response bodies.

use std::pin::Pin;

use bytes::Bytes;
use futures_core::Stream;
use futures_util::StreamExt;

use crate::error::{Error, Result};

pub(crate) type ResponseByteStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>;

pub(crate) async fn collect_bounded(
    mut stream: ResponseByteStream,
    maximum: usize,
) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if chunk.len() > maximum.saturating_sub(body.len()) {
            return Err(Error::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[cfg(test)]
#[path = "_tests_/response_body_tests.rs"]
mod response_body_tests;
