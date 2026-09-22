//! Alternate-backend bounded process behavior.

use futures_util::stream;
use sandbox_interface::{
    BackendRunProcessRequest, BackendStreamProcessRequest, ProcessEventStream, ProcessStreamEvent,
    ProcessStreamOutcome, Result, SandboxProcessOutput,
};

pub(super) fn run(request: BackendRunProcessRequest) -> Result<SandboxProcessOutput> {
    assert_eq!(request.command, "/bin/sh");
    assert_eq!(
        request.args,
        [
            "-c",
            "printf '%s' 'argv-direct'; printf '%s' 'separate-stderr' >&2"
        ]
    );
    Ok(SandboxProcessOutput {
        stdout: b"argv-direct".to_vec(),
        stderr: b"separate-stderr".to_vec(),
        exit_code: Some(0),
        exited: true,
        ..SandboxProcessOutput::default()
    })
}

pub(super) fn stream(request: BackendStreamProcessRequest) -> Result<ProcessEventStream> {
    assert_eq!(request.command, "/bin/sh");
    assert_eq!(
        request.args,
        [
            "-c",
            "printf '%s' 'stream-stdout'; printf '%s' 'stream-stderr' >&2"
        ]
    );
    Ok(Box::pin(stream::iter([
        ProcessStreamEvent::Started { pid: 17 },
        ProcessStreamEvent::Stdout(b"stream-stdout".to_vec()),
        ProcessStreamEvent::Stderr(b"stream-stderr".to_vec()),
        ProcessStreamEvent::Exited { exit_code: 0 },
        ProcessStreamEvent::Outcome(ProcessStreamOutcome::Completed),
    ])))
}
