//! Internal trusted assets shared by provider adapters.

#![warn(unreachable_pub)]

/// Descriptor-relative atomic replacement with a trusted commit fence.
pub const REGULAR_FILE_WRITE: &str = include_str!("regular_file_write.py");
/// Exact replacement reconciliation and atomic writer revocation.
pub const REGULAR_FILE_CLEANUP: &str = include_str!("regular_file_cleanup.py");
