//! Typed Boat command, attach, and protected staging API.

use std::time::Duration;

use async_trait::async_trait;

use crate::{error::Result, transport::runtime::BoatByteStream};

/// Normal synchronous helper result with no output contents in diagnostics.
#[derive(Clone)]
pub struct HelperOutput {
    /// Normal process exit code, absent for signal termination.
    pub exit_code: Option<i32>,
    /// Exact helper stdout within its fixed response bound.
    pub stdout: String,
    /// Whether the provider reports a normal exit without timeout or signal.
    pub normal: bool,
}

/// The provider launcher identity, distinct from the workload PID.
#[derive(Clone, Copy, Debug)]
pub struct DetachedProcess {
    /// Boat's command identifier used for attach and stop.
    pub process_id: u64,
    /// Nonzero launcher operating-system PID.
    pub launcher_pid: u32,
}

/// Offset-preserving attachment to one detached launcher.
#[derive(Clone)]
pub struct AttachRequest {
    /// Provider sandbox identity.
    pub sandbox: String,
    /// Exact command identity returned by start.
    pub process: u64,
    /// Next required stdout byte position.
    pub stdout: u64,
    /// Next required stderr byte position.
    pub stderr: u64,
    /// Remaining absolute execution budget.
    pub timeout: Duration,
}

/// Injectable documented HTTPS runtime API.
#[unimock::unimock(api = BoatRuntimeApiMock)]
#[async_trait]
pub trait BoatRuntimeApi: Send + Sync {
    /// Runs an adapter-generated trusted helper, never arbitrary caller shell text.
    async fn helper(
        &self,
        sandbox: &str,
        command: String,
        timeout: Duration,
    ) -> Result<HelperOutput>;
    /// Dispatches the caller workload exactly once through its trusted launcher.
    async fn start(
        &self,
        sandbox: &str,
        command: String,
        timeout: Duration,
    ) -> Result<DetachedProcess>;
    /// Opens live output at exact offsets without command history.
    async fn attach(&self, request: AttachRequest) -> Result<BoatByteStream>;
    /// Uploads one private native staging chunk, at most 4 MiB decoded.
    async fn upload(
        &self,
        sandbox: &str,
        path: String,
        bytes: Vec<u8>,
        timeout: Duration,
    ) -> Result<()>;
    /// Downloads one private native staging chunk, at most 4 MiB decoded.
    async fn download(&self, sandbox: &str, path: String, timeout: Duration) -> Result<Vec<u8>>;
    /// Attempts to stop one outer launcher without querying retained process state.
    async fn stop(&self, sandbox: &str, process: u64, timeout: Duration) -> Result<()>;
}
