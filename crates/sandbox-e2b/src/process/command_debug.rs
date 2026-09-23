//! Process command diagnostics contain selected metadata only.

use std::fmt;

use super::types::{ProcessCommand, ProcessPtyRequest, SplitProcessCommand, StreamProcessCommand};

impl fmt::Debug for ProcessCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessCommand")
            .field("arg_count", &self.args.len())
            .field("has_cwd", &self.cwd.is_some())
            .field("env_count", &self.envs.len())
            .field("output_capture", &self.output_capture)
            .field("timeout", &self.timeout)
            .field("read_only", &self.read_only)
            .finish()
    }
}

impl fmt::Debug for SplitProcessCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SplitProcessCommand")
            .field("arg_count", &self.args.len())
            .field("has_cwd", &self.cwd.is_some())
            .field("env_count", &self.envs.len())
            .field("stdout_limit", &self.stdout_limit)
            .field("stderr_limit", &self.stderr_limit)
            .field("deadline", &self.deadline)
            .finish()
    }
}

impl fmt::Debug for StreamProcessCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StreamProcessCommand")
            .field("arg_count", &self.args.len())
            .field("stdout_limit", &self.stdout_limit)
            .field("stderr_limit", &self.stderr_limit)
            .field("requested_at", &self.requested_at)
            .field("deadline", &self.deadline)
            .field("idle_timeout", &self.idle_timeout)
            .finish()
    }
}

impl fmt::Debug for ProcessPtyRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessPtyRequest")
            .field("log_limit", &self.log_limit)
            .field("has_cwd", &self.cwd.is_some())
            .finish()
    }
}

#[cfg(test)]
#[path = "_tests_/command_debug_tests.rs"]
mod command_debug_tests;
