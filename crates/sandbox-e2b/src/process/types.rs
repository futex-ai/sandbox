//! Provider-process types and swappable transport trait.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use sandbox_interface::Result;

pub(super) use super::connection::ProcessConnection;
use super::{regular_file_write::ProcessRegularFileWriteRequest, selector::ProcessSelector};

/// Request for one persistent PTY wrapped by a bounded transcript helper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessPtyRequest {
    /// Provider process correlation tag.
    pub tag: String,
    /// Provider-side transcript path.
    pub log_path: String,
    /// Provider-side transcript hard cap.
    pub log_limit: usize,
    /// Unprivileged account used for the interactive login shell.
    pub workload_user: String,
    /// Optional initial working directory.
    pub cwd: Option<String>,
}

/// Non-interactive process command used for maintenance and file ingestion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessCommand {
    /// Executable path.
    pub command: String,
    /// Exact argument vector.
    pub args: Vec<String>,
    /// Optional initial working directory.
    pub cwd: Option<String>,
    /// Explicit output overflow behavior for this command.
    pub output_capture: ProcessOutputCapture,
    /// Maximum command execution duration, capped at 300 seconds.
    pub timeout: Duration,
    /// Whether output overflow maps to the public stateless-read limit error.
    pub read_only: bool,
}

/// Output overflow behavior for one non-interactive provider process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessOutputCapture {
    /// Fail the operation if combined output exceeds the byte limit.
    HardLimit {
        /// Maximum combined output bytes.
        max_bytes: usize,
    },
    /// Keep draining and return only the final combined-output window.
    Tail {
        /// Maximum combined output bytes retained.
        max_bytes: usize,
    },
}

/// Non-interactive process command with separate stream capture bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitProcessCommand {
    /// Executable path or `PATH`-resolved command name.
    pub command: String,
    /// Exact argument vector.
    pub args: Vec<String>,
    /// Maximum captured stdout bytes before overflow is reported.
    pub stdout_limit: usize,
    /// Maximum captured stderr bytes before overflow is reported.
    pub stderr_limit: usize,
    /// Execution deadline for the whole run, capped at 300 seconds.
    pub deadline: Duration,
}

/// Provider process identity and optional consumer tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessInfo {
    /// E2B envd PID.
    pub pid: u32,
    /// Opaque consumer process tag.
    pub tag: Option<String>,
}

/// Bounded result of a non-interactive provider process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessRunOutput {
    /// Combined bounded stdout/stderr bytes.
    pub bytes: Vec<u8>,
    /// Exit code when the command terminalized.
    pub exit_code: Option<i32>,
    /// Whether an end event was observed.
    pub exited: bool,
    /// Whether earlier output bytes were omitted from the returned window.
    pub output_truncated: bool,
}

/// Bounded result of one split-stream non-interactive process run.
///
/// Overflow and deadline expiry are reported as data so the caller can fail
/// typed instead of parsing truncated output.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProcessSplitOutput {
    /// Captured stdout bytes up to the requested limit.
    pub stdout: Vec<u8>,
    /// Captured stderr bytes up to the requested limit.
    pub stderr: Vec<u8>,
    /// Exit code when the process terminalized inside the deadline.
    pub exit_code: Option<i32>,
    /// Whether the process exited normally; signal termination reports false.
    pub exited: bool,
    /// Whether stdout exceeded its capture limit.
    pub stdout_overflowed: bool,
    /// Whether stderr exceeded its capture limit.
    pub stderr_overflowed: bool,
}

/// Bounded transient PTY Connect result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessConnectOutput {
    /// PTY bytes received during the bounded connection.
    pub bytes: Vec<u8>,
    /// Exit code when observed.
    pub exit_code: Option<i32>,
    /// Whether the provider process exited.
    pub exited: bool,
}

/// One durable provider-log read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessFileChunk {
    /// Exact bytes from the requested absolute offset.
    pub bytes: Vec<u8>,
    /// Current provider file size.
    pub total_size: u64,
}

/// Canonical path and type information produced inside the sandbox.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessFileValidation {
    /// Canonical existing transfer root.
    pub canonical_root: String,
    /// Canonical target, following existing symlinks.
    pub canonical_path: String,
    /// Whether the target existed during validation.
    pub exists: bool,
    /// Whether an existing target is a regular file.
    pub regular: bool,
    /// Whether an existing component of the requested path is a symbolic link.
    pub symlink: bool,
    /// Existing target size, or zero for a new target.
    pub size: u64,
}

/// One atomic, bounded regular-file read below a trusted absolute root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessRegularFileRequest {
    /// Absolute directory that contains every eligible target.
    pub root: String,
    /// Normalized root-relative target path.
    pub path: String,
    /// Absolute byte offset within the opened file.
    pub offset: u64,
    /// Maximum number of file bytes returned.
    pub max_bytes: usize,
    /// Maximum provider-side helper duration, capped at 300 seconds.
    pub timeout: Duration,
}

/// Swappable envd process transport used by the E2B adapter.
#[unimock::unimock(api = ProcessTransportMock)]
#[async_trait]
pub trait ProcessTransport: Send + Sync {
    /// Starts one durable PTY and returns its PID.
    async fn start_pty(
        &self,
        connection: ProcessConnection,
        request: ProcessPtyRequest,
    ) -> Result<ProcessInfo>;
    /// Connects transiently to a running PTY for at most 300 seconds.
    async fn connect(
        &self,
        connection: ProcessConnection,
        pid: u32,
        wait: Duration,
        max_bytes: usize,
    ) -> Result<ProcessConnectOutput>;
    /// Runs one bounded non-interactive process.
    async fn run(
        &self,
        connection: ProcessConnection,
        command: ProcessCommand,
    ) -> Result<ProcessRunOutput>;
    /// Runs one bounded non-interactive process with split stream capture.
    async fn run_split(
        &self,
        connection: ProcessConnection,
        command: SplitProcessCommand,
    ) -> Result<ProcessSplitOutput>;
    /// Lists provider processes and their opaque tags.
    async fn list(&self, connection: ProcessConnection) -> Result<Vec<ProcessInfo>>;
    /// Sends exact PTY bytes once.
    async fn send_input(
        &self,
        connection: ProcessConnection,
        selector: ProcessSelector,
        input: Vec<u8>,
    ) -> Result<()>;
    /// Idempotently kills one provider process.
    async fn kill(&self, connection: ProcessConnection, selector: ProcessSelector) -> Result<()>;
    /// Opens and reads one regular file through the same non-following file
    /// descriptor, with every path component resolved below `root`.
    async fn read_regular_file(
        &self,
        connection: ProcessConnection,
        request: ProcessRegularFileRequest,
    ) -> Result<ProcessFileChunk>;
    /// Atomically replaces one regular file through descriptor-relative,
    /// non-following traversal below `root`.
    async fn write_regular_file(
        &self,
        connection: ProcessConnection,
        request: ProcessRegularFileWriteRequest,
    ) -> Result<()>;
    /// Canonicalizes and classifies one transfer target inside the sandbox.
    async fn validate_file(
        &self,
        connection: ProcessConnection,
        root: String,
        path: String,
        allow_missing: bool,
    ) -> Result<ProcessFileValidation>;
    /// Downloads one bounded byte range through envd's file API.
    async fn download_file(
        &self,
        connection: ProcessConnection,
        path: String,
        offset: u64,
        max_bytes: usize,
    ) -> Result<Vec<u8>>;
    /// Replaces one file through envd's file API.
    async fn upload_file(
        &self,
        connection: ProcessConnection,
        path: String,
        bytes: Vec<u8>,
    ) -> Result<()>;
}

/// Shared process transport alias.
pub type DynProcessTransport = Arc<dyn ProcessTransport>;
