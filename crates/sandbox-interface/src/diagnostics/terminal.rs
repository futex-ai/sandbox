//! Terminal diagnostics preserve metadata without displaying input or output.

use std::fmt;

use crate::{
    BackendInputRequest, BackendOutputRequest, BackendTerminal, BackendTerminalCreateRequest,
    BackendTerminalOutput, CreateTerminalRequest, ExecuteTerminalRequest, TranscriptWindow,
    WriteTerminalRequest,
};

impl fmt::Debug for TranscriptWindow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TranscriptWindow")
            .field("terminal_id", &self.terminal_id)
            .field("terminal_state", &self.terminal_state)
            .field("offset", &self.offset)
            .field("next_offset", &self.next_offset)
            .field("revision", &self.revision)
            .field("returned_bytes", &self.returned_bytes)
            .field("truncated", &self.truncated)
            .field("action_id", &self.action_id)
            .field("action_status", &self.action_status)
            .finish()
    }
}

impl fmt::Debug for BackendTerminalOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackendTerminalOutput")
            .field("output_bytes", &self.bytes.len())
            .field("next_offset", &self.next_offset)
            .field("total_size", &self.total_size)
            .field("state", &self.state)
            .field("exit_code", &self.exit_code)
            .field("overflowed", &self.overflowed)
            .finish()
    }
}

impl fmt::Debug for BackendInputRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackendInputRequest")
            .field("input_bytes", &self.input.len())
            .finish()
    }
}

impl fmt::Debug for ExecuteTerminalRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExecuteTerminalRequest")
            .field("owner", &self.owner)
            .field("operation_id", &self.operation_id)
            .field("terminal_id", &self.terminal_id)
            .field("timeout_seconds", &self.timeout_seconds)
            .finish()
    }
}

impl fmt::Debug for WriteTerminalRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WriteTerminalRequest")
            .field("owner", &self.owner)
            .field("sandbox_id", &self.sandbox_id)
            .field("operation_id", &self.operation_id)
            .field("action_id", &self.action_id)
            .field("input_bytes", &self.input.len())
            .field("wait_timeout_seconds", &self.wait_timeout_seconds)
            .finish()
    }
}

impl fmt::Debug for CreateTerminalRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreateTerminalRequest")
            .field("owner", &self.owner)
            .field("operation_id", &self.operation_id)
            .field("lifecycle_operation_id", &self.lifecycle_operation_id)
            .field("sandbox_id", &self.sandbox_id)
            .field("has_cwd", &self.cwd.is_some())
            .finish()
    }
}

impl fmt::Debug for BackendTerminalCreateRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackendTerminalCreateRequest")
            .field("terminal_id", &self.terminal_id)
            .field("operation_id", &self.operation_id)
            .field("has_cwd", &self.cwd.is_some())
            .field("provider_log_limit", &self.provider_log_limit)
            .finish()
    }
}

impl fmt::Debug for BackendTerminal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackendTerminal")
            .field("state", &self.state)
            .finish()
    }
}

impl fmt::Debug for BackendOutputRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackendOutputRequest")
            .field("offset", &self.offset)
            .field("max_bytes", &self.max_bytes)
            .field("provider_log_limit", &self.provider_log_limit)
            .field("wait", &self.wait)
            .finish()
    }
}
