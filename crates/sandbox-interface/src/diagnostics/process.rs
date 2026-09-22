//! Process diagnostics intentionally omit all caller-controlled text and bytes.

use std::fmt;

use crate::{
    BackendReadOnlyExecRequest, BackendRunProcessRequest, ReadOnlyExecOutput, ReadOnlyExecRequest,
    RunProcessRequest, SandboxProcessOutput,
};

impl fmt::Debug for RunProcessRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RunProcessRequest")
            .field("owner", &self.owner)
            .field("sandbox_id", &self.sandbox_id)
            .field("arg_count", &self.args.len())
            .field("has_cwd", &self.cwd.is_some())
            .field("env_count", &self.envs.len())
            .field("stdout_limit", &self.stdout_limit)
            .field("stderr_limit", &self.stderr_limit)
            .field("deadline", &self.deadline)
            .finish()
    }
}

impl fmt::Debug for BackendRunProcessRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackendRunProcessRequest")
            .field("arg_count", &self.args.len())
            .field("has_cwd", &self.cwd.is_some())
            .field("env_count", &self.envs.len())
            .field("stdout_limit", &self.stdout_limit)
            .field("stderr_limit", &self.stderr_limit)
            .field("deadline", &self.deadline)
            .finish()
    }
}

impl fmt::Debug for SandboxProcessOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SandboxProcessOutput")
            .field("stdout_bytes", &self.stdout.len())
            .field("stderr_bytes", &self.stderr.len())
            .field("exit_code", &self.exit_code)
            .field("exited", &self.exited)
            .field("stdout_overflowed", &self.stdout_overflowed)
            .field("stderr_overflowed", &self.stderr_overflowed)
            .finish()
    }
}

impl fmt::Debug for ReadOnlyExecRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReadOnlyExecRequest")
            .field("owner", &self.owner)
            .field("sandbox_id", &self.sandbox_id)
            .field("arg_count", &self.args.len())
            .field("output_limit", &self.output_limit)
            .field("timeout_seconds", &self.timeout_seconds)
            .finish()
    }
}

impl fmt::Debug for BackendReadOnlyExecRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackendReadOnlyExecRequest")
            .field("arg_count", &self.args.len())
            .field("output_limit", &self.output_limit)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl fmt::Debug for ReadOnlyExecOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReadOnlyExecOutput")
            .field("output_bytes", &self.bytes.len())
            .field("exit_code", &self.exit_code)
            .finish()
    }
}
