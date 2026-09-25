//! Process diagnostics intentionally omit all caller-controlled text and bytes.

use std::fmt;

use crate::{
    BackendReadOnlyExecRequest, BackendRunProcessRequest, BackendStreamProcessRequest,
    ProcessStreamEvent, ReadOnlyExecOutput, ReadOnlyExecRequest, RunProcessRequest,
    SandboxProcessOutput, StreamProcessRequest,
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

impl fmt::Debug for StreamProcessRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StreamProcessRequest")
            .field("owner", &self.owner)
            .field("sandbox_id", &self.sandbox_id)
            .field("arg_count", &self.args.len())
            .field("stdout_limit", &self.stdout_limit)
            .field("stderr_limit", &self.stderr_limit)
            .field("deadline", &self.deadline)
            .field("idle_timeout", &self.idle_timeout)
            .finish()
    }
}

impl fmt::Debug for BackendStreamProcessRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackendStreamProcessRequest")
            .field("arg_count", &self.args.len())
            .field("stdout_limit", &self.stdout_limit)
            .field("stderr_limit", &self.stderr_limit)
            .field("deadline", &self.deadline)
            .field("idle_timeout", &self.idle_timeout)
            .finish()
    }
}

impl fmt::Debug for ProcessStreamEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Started { pid } => formatter.debug_struct("Started").field("pid", pid).finish(),
            Self::Stdout(bytes) => formatter
                .debug_struct("Stdout")
                .field("output_bytes", &bytes.len())
                .finish(),
            Self::Stderr(bytes) => formatter
                .debug_struct("Stderr")
                .field("output_bytes", &bytes.len())
                .finish(),
            Self::Exited { exit_code, exited } => formatter
                .debug_struct("Exited")
                .field("exit_code", exit_code)
                .field("exited", exited)
                .finish(),
            Self::Outcome(outcome) => formatter.debug_tuple("Outcome").field(outcome).finish(),
        }
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
