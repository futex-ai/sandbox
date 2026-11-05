//! Documented runtime envelopes over a bounded credentialed HTTP boundary.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    error::{Error, Result},
    transport::{
        runtime::{BoatByteStream, BoatRuntimeRequest, BoatRuntimeTransport},
        runtime_route::{RuntimeRoute, valid_stage_path},
    },
};

use super::api::{AttachRequest, BoatRuntimeApi, DetachedProcess, HelperOutput};

/// Production runtime API retaining the transport's origin and credential policy.
pub struct HttpsBoatRuntimeApi {
    transport: Arc<dyn BoatRuntimeTransport>,
}

impl HttpsBoatRuntimeApi {
    /// Composes documented runtime calls over a credentialed transport.
    #[must_use]
    pub fn new(transport: Arc<dyn BoatRuntimeTransport>) -> Self {
        Self { transport }
    }

    async fn command<T: DeserializeOwned>(
        &self,
        sandbox: &str,
        command: String,
        detached: bool,
        timeout: Duration,
    ) -> Result<T> {
        let body = encode(&Command {
            command,
            detached,
            stream: false,
            timeout_seconds: timeout.as_secs().clamp(1, 600),
        })?;
        let bytes = self
            .transport
            .request(BoatRuntimeRequest {
                route: RuntimeRoute::Commands(sandbox.to_owned()),
                body: Some(body),
                timeout,
                response_limit: 64 * 1024,
            })
            .await?;
        decode(&bytes)
    }
}

#[async_trait]
impl BoatRuntimeApi for HttpsBoatRuntimeApi {
    async fn helper(
        &self,
        sandbox: &str,
        command: String,
        timeout: Duration,
    ) -> Result<HelperOutput> {
        let result: Finished = self.command(sandbox, command, false, timeout).await?;
        if !result.ok
            || result.kind != "command.finished"
            || !result.stderr.is_empty()
            || result.stdout_truncated
            || result.stderr_truncated
        {
            return Err(Error::InvalidResponse);
        }
        Ok(HelperOutput {
            exit_code: result.exit_code,
            stdout: result.stdout,
            normal: !result.timed_out
                && result.signal.is_none()
                && result.exit_code.is_some()
                && result.success == (result.exit_code == Some(0)),
        })
    }
    async fn start(
        &self,
        sandbox: &str,
        command: String,
        timeout: Duration,
    ) -> Result<DetachedProcess> {
        let result: Started = self.command(sandbox, command, true, timeout).await?;
        if !result.ok
            || !result.success
            || result.kind != "command.started"
            || result.process_id == 0
            || result.pid == 0
        {
            return Err(Error::InvalidResponse);
        }
        Ok(DetachedProcess {
            process_id: result.process_id,
            launcher_pid: result.pid,
        })
    }
    async fn attach(&self, request: AttachRequest) -> Result<BoatByteStream> {
        self.transport
            .attach(BoatRuntimeRequest {
                route: RuntimeRoute::Attach {
                    sandbox: request.sandbox,
                    process: request.process,
                    stdout: request.stdout,
                    stderr: request.stderr,
                },
                body: None,
                timeout: request.timeout,
                response_limit: 0,
            })
            .await
    }
    async fn upload(
        &self,
        sandbox: &str,
        path: String,
        bytes: Vec<u8>,
        timeout: Duration,
    ) -> Result<()> {
        if !valid_stage_path(&path) || bytes.len() > 4 * 1024 * 1024 {
            return Err(Error::InvalidRequest);
        }
        let length = bytes.len();
        let body = encode(&Upload {
            path: &path,
            encoding: "base64",
            content: STANDARD.encode(bytes),
        })?;
        let bytes = self
            .transport
            .request(BoatRuntimeRequest {
                route: RuntimeRoute::Upload {
                    sandbox: sandbox.to_owned(),
                },
                body: Some(body),
                timeout,
                response_limit: 4096,
            })
            .await?;
        let result: FileReply = decode(&bytes)?;
        if !result.ok
            || !result.success
            || result.kind != "file.written"
            || result.path != path
            || result.encoding != "base64"
            || result.size != length
        {
            return Err(Error::InvalidResponse);
        }
        Ok(())
    }
    async fn download(&self, sandbox: &str, path: String, timeout: Duration) -> Result<Vec<u8>> {
        let bytes = self
            .transport
            .request(BoatRuntimeRequest {
                route: RuntimeRoute::Download {
                    sandbox: sandbox.to_owned(),
                    path: path.clone(),
                },
                body: None,
                timeout,
                response_limit: 6 * 1024 * 1024,
            })
            .await?;
        let result: FileReply = decode(&bytes)?;
        if !result.ok
            || !result.success
            || result.kind != "file.read"
            || result.path != path
            || result.encoding != "base64"
            || result.size > 4 * 1024 * 1024
        {
            return Err(Error::InvalidResponse);
        }
        let decoded = match result.content.and_then(|s| STANDARD.decode(s).ok()) {
            Some(bytes) => bytes,
            None => return Err(Error::InvalidResponse),
        };
        if decoded.len() != result.size {
            return Err(Error::InvalidResponse);
        }
        Ok(decoded)
    }
    async fn stop(&self, sandbox: &str, process: u64, timeout: Duration) -> Result<()> {
        let _ = self
            .transport
            .request(BoatRuntimeRequest {
                route: RuntimeRoute::Stop {
                    sandbox: sandbox.to_owned(),
                    process,
                },
                body: None,
                timeout,
                response_limit: 64 * 1024,
            })
            .await?;
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Command {
    command: String,
    detached: bool,
    stream: bool,
    timeout_seconds: u64,
}
#[derive(Serialize)]
struct Upload<'a> {
    path: &'a str,
    encoding: &'static str,
    content: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Finished {
    ok: bool,
    success: bool,
    #[serde(rename = "type")]
    kind: String,
    exit_code: Option<i32>,
    signal: Option<String>,
    stdout: String,
    stderr: String,
    timed_out: bool,
    #[serde(default)]
    stdout_truncated: bool,
    #[serde(default)]
    stderr_truncated: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Started {
    ok: bool,
    #[serde(rename = "type")]
    kind: String,
    success: bool,
    process_id: u64,
    pid: u32,
}
#[derive(Deserialize)]
struct FileReply {
    ok: bool,
    #[serde(rename = "type")]
    kind: String,
    success: bool,
    path: String,
    encoding: String,
    size: usize,
    content: Option<String>,
}

pub(crate) fn encode(value: &impl Serialize) -> Result<Vec<u8>> {
    match serde_json::to_vec(value) {
        Ok(bytes) => Ok(bytes),
        Err(source) => Err(Error::internal_with(source, "encode Boat runtime payload")),
    }
}
pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    match serde_json::from_slice(bytes) {
        Ok(value) => Ok(value),
        Err(_) => Err(Error::InvalidResponse),
    }
}
