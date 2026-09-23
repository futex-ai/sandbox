//! Provider-neutral incremental process conformance probe.

use std::time::Duration;

use futures_util::StreamExt;

use crate::{
    BackendStreamProcessRequest, Error, ProcessStreamEvent, ProcessStreamOutcome, ProviderRef,
    Result, SandboxBackend,
};

const STREAM_PROCESS_SCRIPT: &str = "printf '%s' 'stream-stdout'; printf '%s' 'stream-stderr' >&2";

pub(crate) async fn exercise(
    backend: &dyn SandboxBackend,
    sandbox_provider_ref: ProviderRef,
) -> Result<()> {
    let mut stream = backend
        .stream_process(BackendStreamProcessRequest {
            sandbox_provider_ref,
            command: "/bin/sh".to_owned(),
            args: vec!["-c".to_owned(), STREAM_PROCESS_SCRIPT.to_owned()],
            stdout_limit: 4096,
            stderr_limit: 1024,
            deadline: Duration::from_secs(60),
            idle_timeout: Duration::from_secs(10),
        })
        .await?;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut started = false;
    let mut exited = false;
    let mut completed = false;
    while let Some(event) = stream.next().await {
        if completed {
            return Err(Error::internal_message(
                "backend process stream emitted an event after its outcome",
            ));
        }
        match event {
            ProcessStreamEvent::Started { pid } if !started && pid != 0 => started = true,
            ProcessStreamEvent::Stdout(bytes) if started && !exited => {
                stdout.extend_from_slice(&bytes);
            }
            ProcessStreamEvent::Stderr(bytes) if started && !exited => {
                stderr.extend_from_slice(&bytes);
            }
            ProcessStreamEvent::Exited {
                exit_code,
                exited: true,
            } if started && !exited && exit_code == 0 => {
                exited = true;
            }
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::Completed) if exited => {
                completed = true;
            }
            _ => {
                return Err(Error::internal_message(
                    "backend process stream changed event ordering or outcome",
                ));
            }
        }
    }
    if !completed || stdout != b"stream-stdout" || stderr != b"stream-stderr" {
        return Err(Error::internal_message(
            "backend process stream changed split output",
        ));
    }
    Ok(())
}
