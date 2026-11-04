//! Bounded regular-file reads and safe atomic replacement writes.

use async_trait::async_trait;

use crate::{BackendFileContent, BackendReadFileRequest, BackendWriteFileRequest, Result};

/// Bounded regular-file reads and safe atomic replacement writes.
#[unimock::unimock(api = SandboxFilesMock)]
#[async_trait]
pub trait SandboxFiles: Send + Sync {
    /// Reads a bounded regular file without exposing provider credentials.
    async fn read_file(&self, request: BackendReadFileRequest) -> Result<BackendFileContent>;
    /// Replaces a bounded regular file without exposing provider credentials.
    async fn write_file(&self, request: BackendWriteFileRequest) -> Result<()>;
}
