//! Image requests expose only identifiers and counts when formatted.

use std::fmt;

use crate::{BackendPrepareImageRequest, RealizeImageFileInput, RealizeImageRequest};

impl fmt::Debug for BackendPrepareImageRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackendPrepareImageRequest")
            .field("sandbox_id", &self.sandbox_id)
            .field("owner", &self.owner)
            .field("input_file_count", &self.input_files.len())
            .field("verify_command_count", &self.verify_commands.len())
            .finish()
    }
}

impl fmt::Debug for RealizeImageRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RealizeImageRequest")
            .field("owner", &self.owner)
            .field("operation_id", &self.operation_id)
            .field("input_file_count", &self.input_files.len())
            .field("verify_command_count", &self.verify_commands.len())
            .finish()
    }
}

impl fmt::Debug for RealizeImageFileInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RealizeImageFileInput")
            .field("input_bytes", &self.bytes.len())
            .finish()
    }
}
