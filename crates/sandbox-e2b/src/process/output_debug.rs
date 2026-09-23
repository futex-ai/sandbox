//! Process output diagnostics never render the underlying captured bytes.

use std::fmt;

use super::framing::ProcessEvent;
use super::types::{ProcessConnectOutput, ProcessFileChunk, ProcessRunOutput, ProcessSplitOutput};

impl fmt::Debug for ProcessRunOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessRunOutput")
            .field("output_bytes", &self.bytes.len())
            .field("exit_code", &self.exit_code)
            .field("exited", &self.exited)
            .field("output_truncated", &self.output_truncated)
            .finish()
    }
}

impl fmt::Debug for ProcessSplitOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessSplitOutput")
            .field("stdout_bytes", &self.stdout.len())
            .field("stderr_bytes", &self.stderr.len())
            .field("exit_code", &self.exit_code)
            .field("exited", &self.exited)
            .field("stdout_overflowed", &self.stdout_overflowed)
            .field("stderr_overflowed", &self.stderr_overflowed)
            .finish()
    }
}

impl fmt::Debug for ProcessConnectOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessConnectOutput")
            .field("output_bytes", &self.bytes.len())
            .field("exit_code", &self.exit_code)
            .field("exited", &self.exited)
            .finish()
    }
}

impl fmt::Debug for ProcessFileChunk {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessFileChunk")
            .field("output_bytes", &self.bytes.len())
            .field("total_size", &self.total_size)
            .finish()
    }
}

impl fmt::Debug for ProcessEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(pid) => formatter.debug_tuple("Start").field(pid).finish(),
            Self::Data { channel, bytes } => formatter
                .debug_struct("Data")
                .field("channel", channel)
                .field("output_bytes", &bytes.len())
                .finish(),
            Self::End { exit_code, exited } => formatter
                .debug_struct("End")
                .field("exit_code", exit_code)
                .field("exited", exited)
                .finish(),
            Self::KeepAlive => formatter.write_str("KeepAlive"),
        }
    }
}

#[cfg(test)]
#[path = "_tests_/output_debug_tests.rs"]
mod output_debug_tests;
