//! Bounded argv-direct non-interactive process execution values.

use std::{collections::BTreeMap, time::Duration};

use crate::{Error, FILE_TRANSFER_PATH_MAX_BYTES, ProviderRef, ResourceOwner, Result, SandboxId};

/// Maximum bytes accepted for one captured process stream.
pub const PROCESS_RUN_MAX_STREAM_BYTES: usize = 64 * 1024 * 1024;

/// Maximum total UTF-8 bytes across the command and every argument.
pub const PROCESS_RUN_MAX_ARGV_BYTES: usize = 128 * 1024;

/// Maximum UTF-8 bytes across direct-process environment names and values.
pub const PROCESS_RUN_MAX_ENV_BYTES: usize = 64 * 1024;

/// Maximum environment entries accepted for one direct process.
pub const PROCESS_RUN_MAX_ENV_VARS: usize = 256;

/// Maximum execution deadline accepted for one process run.
pub const PROCESS_RUN_MAX_DEADLINE: Duration = Duration::from_secs(300);

/// Typed reason that a direct-process execution context was rejected.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProcessRunContextError {
    /// The working directory is not an absolute control-free path.
    #[error("[sandbox_interface/process_run] working directory is invalid")]
    InvalidWorkingDirectory,
    /// The working directory exceeds the shared path byte limit.
    #[error("[sandbox_interface/process_run] working directory exceeds the {limit}-byte limit")]
    WorkingDirectoryTooLarge {
        /// Maximum UTF-8 byte length.
        limit: usize,
    },
    /// An environment name does not use the portable identifier shape.
    #[error("[sandbox_interface/process_run] environment variable name is invalid")]
    InvalidEnvironmentName,
    /// An environment value contains a NUL byte.
    #[error("[sandbox_interface/process_run] environment variable value is invalid")]
    InvalidEnvironmentValue,
    /// The sandbox template owns this environment name.
    #[error("[sandbox_interface/process_run] environment variable name is template-owned")]
    TemplateOwnedEnvironmentName,
    /// The environment contains too many entries.
    #[error("[sandbox_interface/process_run] environment exceeds the {limit}-variable limit")]
    TooManyEnvironmentVariables {
        /// Maximum environment entry count.
        limit: usize,
    },
    /// Environment names and values exceed their combined byte limit.
    #[error("[sandbox_interface/process_run] environment exceeds the {limit}-byte limit")]
    EnvironmentTooLarge {
        /// Maximum combined UTF-8 byte count.
        limit: usize,
    },
}

/// Request to run one bounded non-interactive process in an owned sandbox.
///
/// Execution is argv-direct and shell-free: the command and arguments are
/// passed to the provider verbatim, with no shell interpretation, PTY,
/// terminal, or transcript row. Trusted services use this boundary for driver
/// and helper invocations; it is never model-reachable.
#[derive(Clone, Eq, PartialEq)]
pub struct RunProcessRequest {
    /// Caller resource owner.
    pub owner: ResourceOwner,
    /// Stable sandbox handle.
    pub sandbox_id: SandboxId,
    /// Executable path or `PATH`-resolved command name.
    pub command: String,
    /// Exact argument vector.
    pub args: Vec<String>,
    /// Optional absolute initial working directory.
    pub cwd: Option<String>,
    /// Explicit environment additions; values are secret in diagnostics.
    pub envs: BTreeMap<String, String>,
    /// Maximum captured stdout bytes before the run reports overflow.
    pub stdout_limit: usize,
    /// Maximum captured stderr bytes before the run reports overflow.
    pub stderr_limit: usize,
    /// Execution deadline; expiry returns a non-exited output, never a hang.
    pub deadline: Duration,
}

/// Provider request to run one bounded non-interactive process.
///
/// Backends must reject an empty command, combined command and argument bytes
/// above [`PROCESS_RUN_MAX_ARGV_BYTES`], stream limits above
/// [`PROCESS_RUN_MAX_STREAM_BYTES`], and deadlines above
/// [`PROCESS_RUN_MAX_DEADLINE`] before contacting their provider.
#[derive(Clone, Eq, PartialEq)]
pub struct BackendRunProcessRequest {
    /// Source provider sandbox reference.
    pub sandbox_provider_ref: ProviderRef,
    /// Executable path or `PATH`-resolved command name.
    pub command: String,
    /// Exact argument vector.
    pub args: Vec<String>,
    /// Optional absolute initial working directory.
    pub cwd: Option<String>,
    /// Explicit environment additions; values are secret in diagnostics.
    pub envs: BTreeMap<String, String>,
    /// Maximum captured stdout bytes before the run reports overflow.
    pub stdout_limit: usize,
    /// Maximum captured stderr bytes before the run reports overflow.
    pub stderr_limit: usize,
    /// Execution deadline; expiry returns a non-exited output, never a hang.
    pub deadline: Duration,
}

impl RunProcessRequest {
    /// Validates the caller-controlled working directory and environment map.
    pub fn validate_execution_context(&self) -> Result<()> {
        validate_execution_context(self.cwd.as_deref(), &self.envs)
    }
}

impl BackendRunProcessRequest {
    /// Validates the caller-controlled working directory and environment map.
    pub fn validate_execution_context(&self) -> Result<()> {
        validate_execution_context(self.cwd.as_deref(), &self.envs)
    }
}

fn validate_execution_context(cwd: Option<&str>, envs: &BTreeMap<String, String>) -> Result<()> {
    validate_working_directory(cwd)?;
    if envs.len() > PROCESS_RUN_MAX_ENV_VARS {
        return Err(invalid_context(
            ProcessRunContextError::TooManyEnvironmentVariables {
                limit: PROCESS_RUN_MAX_ENV_VARS,
            },
        ));
    }
    let mut total_bytes = 0usize;
    for (name, value) in envs {
        if !valid_environment_name(name) {
            return Err(invalid_context(
                ProcessRunContextError::InvalidEnvironmentName,
            ));
        }
        if template_owns_environment_name(name) {
            return Err(invalid_context(
                ProcessRunContextError::TemplateOwnedEnvironmentName,
            ));
        }
        if value.contains('\0') {
            return Err(invalid_context(
                ProcessRunContextError::InvalidEnvironmentValue,
            ));
        }
        let Some(with_name) = total_bytes.checked_add(name.len()) else {
            return Err(environment_too_large());
        };
        let Some(with_value) = with_name.checked_add(value.len()) else {
            return Err(environment_too_large());
        };
        if with_value > PROCESS_RUN_MAX_ENV_BYTES {
            return Err(environment_too_large());
        }
        total_bytes = with_value;
    }
    Ok(())
}

fn validate_working_directory(cwd: Option<&str>) -> Result<()> {
    let Some(cwd) = cwd else {
        return Ok(());
    };
    if cwd.len() > FILE_TRANSFER_PATH_MAX_BYTES {
        return Err(invalid_context(
            ProcessRunContextError::WorkingDirectoryTooLarge {
                limit: FILE_TRANSFER_PATH_MAX_BYTES,
            },
        ));
    }
    if !cwd.starts_with('/') || cwd.chars().any(char::is_control) {
        return Err(invalid_context(
            ProcessRunContextError::InvalidWorkingDirectory,
        ));
    }
    Ok(())
}

fn valid_environment_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    matches!(bytes.next(), Some(b'A'..=b'Z' | b'a'..=b'z' | b'_'))
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn template_owns_environment_name(name: &str) -> bool {
    matches!(name, "PATH" | "HOME" | "LD_PRELOAD" | "LD_LIBRARY_PATH")
        || name.starts_with("LD_")
        || name.starts_with("DYLD_")
}

fn environment_too_large() -> Error {
    invalid_context(ProcessRunContextError::EnvironmentTooLarge {
        limit: PROCESS_RUN_MAX_ENV_BYTES,
    })
}

fn invalid_context(reason: ProcessRunContextError) -> Error {
    Error::InvalidProcessRunContext { reason }
}

/// Bounded result of one non-interactive process run.
///
/// Overflow and deadline expiry are reported as data, never silently
/// truncated into a success: callers must treat `stdout_overflowed`,
/// `stderr_overflowed`, and a missing exit as typed failures before parsing
/// any captured bytes.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct SandboxProcessOutput {
    /// Captured stdout bytes up to the requested limit.
    pub stdout: Vec<u8>,
    /// Captured stderr bytes up to the requested limit.
    pub stderr: Vec<u8>,
    /// Exit code when the process terminalized inside the deadline.
    pub exit_code: Option<i32>,
    /// Whether an end event was observed inside the deadline.
    pub exited: bool,
    /// Whether stdout exceeded its capture limit.
    pub stdout_overflowed: bool,
    /// Whether stderr exceeded its capture limit.
    pub stderr_overflowed: bool,
}

#[cfg(test)]
#[path = "_tests_/process_run_tests.rs"]
mod process_run_tests;
