//! Streaming HTTP response bound coverage.

use bytes::Bytes;

use crate::error::Error;

use super::{ResponseByteStream, collect_bounded};

#[tokio::test]
async fn cumulative_chunk_size_is_rejected_before_the_body_can_grow_past_its_limit() {
    let stream: ResponseByteStream = Box::pin(futures_util::stream::iter([
        Ok(Bytes::from_static(b"123")),
        Ok(Bytes::from_static(b"456")),
    ]));

    let result = collect_bounded(stream, 5).await;

    assert!(matches!(result, Err(Error::ResponseTooLarge)));
}

#[tokio::test]
async fn chunks_at_the_exact_cumulative_limit_are_returned() {
    let stream: ResponseByteStream = Box::pin(futures_util::stream::iter([
        Ok(Bytes::from_static(b"123")),
        Ok(Bytes::from_static(b"45")),
    ]));

    let body = collect_bounded(stream, 5).await.expect("bounded body");

    assert_eq!(body, b"12345");
}
