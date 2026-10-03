//! File-write diagnostics omit caller paths, provider references, and contents.

use std::fmt;

use crate::{BackendWriteFileRequest, WriteFileRequest};

impl fmt::Debug for WriteFileRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WriteFileRequest")
            .field("lifecycle_operation_id", &self.lifecycle_operation_id)
            .field("owner", &self.owner)
            .field("sandbox_id", &self.sandbox_id)
            .field("input_bytes", &self.bytes.len())
            .finish()
    }
}

impl fmt::Debug for BackendWriteFileRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackendWriteFileRequest")
            .field("input_bytes", &self.bytes.len())
            .finish()
    }
}
