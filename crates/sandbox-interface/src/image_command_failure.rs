//! Metadata-only command failures for platform image realization.

use serde::{Deserialize, Serialize};

/// Provider-neutral failure facts from one user-authored image command.
///
/// Captured output is sensitive process data and is never part of an error.
/// These facts preserve completion and capture state without inspecting,
/// normalizing, or masking the command's bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ImageCommandFailure {
    /// Observed process exit code, when the backend received one.
    pub exit_code: Option<i32>,
    /// Whether normal process exit was observed.
    pub exited: bool,
    /// Raw bytes retained by capture, excluding any discarded earlier output.
    pub output_bytes: usize,
    /// Whether capture discarded earlier bytes to enforce its bound.
    pub output_truncated: bool,
}

#[cfg(test)]
#[path = "_tests_/image_command_failure_tests.rs"]
mod image_command_failure_tests;
