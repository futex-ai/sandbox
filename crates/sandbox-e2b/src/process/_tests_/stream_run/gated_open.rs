//! A controllable asynchronous open delegating all provider calls to unimock.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use tokio::sync::Notify;

use crate::error::Result;
use crate::process::{
    connect::ConnectProcessTransport,
    http::{ByteStream, ConnectHttpTransport},
    types::ProcessConnection,
};

pub(super) fn transport(
    mock: unimock::Unimock,
    entered: Arc<Notify>,
    resume: Arc<Notify>,
) -> ConnectProcessTransport {
    ConnectProcessTransport {
        http: Arc::new(GatedOpen {
            inner: Arc::new(mock),
            entered,
            resume,
        }),
        backend_id: "configured-e2b".to_owned(),
    }
}

struct GatedOpen {
    inner: Arc<dyn ConnectHttpTransport>,
    entered: Arc<Notify>,
    resume: Arc<Notify>,
}

#[async_trait]
impl ConnectHttpTransport for GatedOpen {
    async fn stream(
        &self,
        connection: ProcessConnection,
        method: String,
        request_json: Vec<u8>,
    ) -> Result<ByteStream> {
        self.inner.stream(connection, method, request_json).await
    }

    async fn stream_with_timeout(
        &self,
        connection: ProcessConnection,
        method: String,
        request_json: Vec<u8>,
        request_timeout: Duration,
    ) -> Result<ByteStream> {
        self.entered.notify_one();
        self.resume.notified().await;
        self.inner
            .stream_with_timeout(connection, method, request_json, request_timeout)
            .await
    }

    async fn unary(
        &self,
        connection: ProcessConnection,
        method: String,
        request_json: Vec<u8>,
        ambiguous: bool,
    ) -> Result<Vec<u8>> {
        self.inner
            .unary(connection, method, request_json, ambiguous)
            .await
    }

    async fn download(
        &self,
        connection: ProcessConnection,
        path: String,
        offset: u64,
        max_bytes: usize,
    ) -> Result<Vec<u8>> {
        self.inner
            .download(connection, path, offset, max_bytes)
            .await
    }

    async fn upload(
        &self,
        connection: ProcessConnection,
        path: String,
        bytes: Vec<u8>,
    ) -> Result<()> {
        self.inner.upload(connection, path, bytes).await
    }
}
