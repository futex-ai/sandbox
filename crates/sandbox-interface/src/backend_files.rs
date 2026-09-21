//! Provider-neutral file-transfer backend values.

use crate::ProviderRef;

/// Provider request to read a bounded regular file below one root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendReadFileRequest {
    /// Source provider sandbox reference.
    pub sandbox_provider_ref: ProviderRef,
    /// Absolute transfer root selected by the trusted caller.
    pub root: String,
    /// Root-relative normalized file path.
    pub path: String,
    /// Absolute byte offset within the file.
    pub offset: u64,
    /// Maximum bytes returned by this call.
    pub max_bytes: usize,
}

/// Provider request to replace one bounded regular file below one root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendWriteFileRequest {
    /// Source provider sandbox reference.
    pub sandbox_provider_ref: ProviderRef,
    /// Absolute transfer root selected by the trusted caller.
    pub root: String,
    /// Root-relative normalized file path.
    pub path: String,
    /// Exact replacement bytes.
    pub bytes: Vec<u8>,
}

/// Bounded provider file-read result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendFileContent {
    /// Exact requested bytes.
    pub bytes: Vec<u8>,
    /// Complete regular-file size observed by the provider.
    pub total_size: u64,
}
