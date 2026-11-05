//! Paused-clock barriers at runtime operations and first attached bytes.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use futures_util::{StreamExt, stream};
use tokio::sync::Notify;

use crate::{
    error::{Error, Result},
    runtime::api::{AttachRequest, BoatRuntimeApi, DetachedProcess, HelperOutput},
    transport::runtime::BoatByteStream,
};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Point {
    Bootstrap,
    Upload,
    Frame,
    Release,
}

pub(super) struct Gate {
    pub(super) point: Point,
    pub(super) entered: Notify,
    pub(super) resume: Notify,
    pub(super) fail: bool,
    pub(super) armed: AtomicBool,
}

impl Gate {
    async fn wait(&self, point: Point) -> Result<()> {
        if self.point == point && self.armed.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.resume.notified().await;
            if self.fail {
                return Err(Error::Unavailable);
            }
        }
        Ok(())
    }
}

pub(super) struct GatedApi {
    pub(super) inner: Arc<dyn BoatRuntimeApi>,
    pub(super) gate: Arc<Gate>,
}

#[async_trait]
impl BoatRuntimeApi for GatedApi {
    async fn helper(
        &self,
        sandbox: &str,
        command: String,
        timeout: Duration,
    ) -> Result<HelperOutput> {
        let point = if command.ends_with(" release") {
            Some(Point::Release)
        } else if !command.ends_with(" clean") && !command.ends_with(" terminate") {
            Some(Point::Bootstrap)
        } else {
            None
        };
        let output = self.inner.helper(sandbox, command, timeout).await?;
        if let Some(point) = point {
            self.gate.wait(point).await?;
        }
        Ok(output)
    }

    async fn start(
        &self,
        sandbox: &str,
        command: String,
        timeout: Duration,
    ) -> Result<DetachedProcess> {
        self.inner.start(sandbox, command, timeout).await
    }

    async fn attach(&self, request: AttachRequest) -> Result<BoatByteStream> {
        let stream = self.inner.attach(request).await?;
        let gate = self.gate.clone();
        Ok(Box::pin(stream::unfold(
            (stream, gate, true),
            |(mut stream, gate, first)| async move {
                if first && let Err(error) = gate.wait(Point::Frame).await {
                    return Some((Err(error), (stream, gate, false)));
                }
                stream
                    .next()
                    .await
                    .map(|bytes| (bytes, (stream, gate, false)))
            },
        )))
    }

    async fn upload(
        &self,
        sandbox: &str,
        path: String,
        bytes: Vec<u8>,
        timeout: Duration,
    ) -> Result<()> {
        self.inner.upload(sandbox, path, bytes, timeout).await?;
        self.gate.wait(Point::Upload).await
    }

    async fn download(&self, sandbox: &str, path: String, timeout: Duration) -> Result<Vec<u8>> {
        self.inner.download(sandbox, path, timeout).await
    }

    async fn stop(&self, sandbox: &str, process: u64, timeout: Duration) -> Result<()> {
        self.inner.stop(sandbox, process, timeout).await
    }
}
