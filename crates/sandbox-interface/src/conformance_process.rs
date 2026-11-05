//! Collected and streaming process probes with explicit process capability.

use std::{collections::BTreeMap, time::Duration};

use crate::{
    BackendRunProcessRequest, Error, ProviderRef, Result, SandboxProcesses,
    conformance_process_stream,
};

const PROCESS_CWD: &str = "/workspace";
const PROCESS_ENV_VALUE: &str = "environment-map";
const PROCESS_SCRIPT: &str = "pwd; printf %s \"$SANDBOX_PROBE\"; printf %s 'separate-stderr' >&2";
const PROCESS_STDERR: &[u8] = b"separate-stderr";

/// Proves bounded collected and streaming output with exact execution context.
///
/// The caller owns the sandbox. The image must provide `/bin/sh` and `/workspace`.
pub async fn exercise_processes(
    backend: &dyn SandboxProcesses,
    sandbox_provider_ref: ProviderRef,
) -> Result<()> {
    let process = backend
        .run_process(BackendRunProcessRequest {
            sandbox_provider_ref: sandbox_provider_ref.clone(),
            command: "/bin/sh".to_owned(),
            args: vec!["-c".to_owned(), PROCESS_SCRIPT.to_owned()],
            cwd: Some(PROCESS_CWD.to_owned()),
            envs: BTreeMap::from([("SANDBOX_PROBE".to_owned(), PROCESS_ENV_VALUE.to_owned())]),
            stdout_limit: 4096,
            stderr_limit: 1024,
            deadline: Duration::from_secs(60),
        })
        .await?;
    if process.stdout != b"/workspace\nenvironment-map"
        || process.stderr != PROCESS_STDERR
        || process.exit_code != Some(0)
        || !process.exited
        || process.stdout_overflowed
        || process.stderr_overflowed
    {
        return Err(Error::internal_message(
            "backend process run changed bounded split output",
        ));
    }
    conformance_process_stream::exercise(backend, sandbox_provider_ref.clone()).await?;

    Ok(())
}
