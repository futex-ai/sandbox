//! Alternate-backend bounded process behavior.

use std::collections::BTreeMap;

use futures_util::stream;
use sandbox_interface::{
    BackendRunProcessRequest, BackendStreamProcessRequest, ProcessEventStream, ProcessStreamEvent,
    ProcessStreamOutcome, Result, SandboxProcessOutput,
};

pub(super) fn run(request: BackendRunProcessRequest) -> Result<SandboxProcessOutput> {
    assert_eq!(request.command, "/bin/sh");
    assert_eq!(request.args.first().map(String::as_str), Some("-c"));
    match request.args.get(1).map(String::as_str) {
        Some("pwd; printf %s \"$SANDBOX_PROBE\"; printf %s 'separate-stderr' >&2") => {
            assert_eq!(request.cwd.as_deref(), Some("/workspace"));
            assert_eq!(
                request.envs,
                BTreeMap::from([("SANDBOX_PROBE".to_owned(), "environment-map".to_owned())])
            );
            Ok(SandboxProcessOutput {
                stdout: b"/workspace\nenvironment-map".to_vec(),
                stderr: b"separate-stderr".to_vec(),
                exit_code: Some(0),
                exited: true,
                ..SandboxProcessOutput::default()
            })
        }
        Some(script) if script.ends_with("https://example.com/") => {
            assert!(request.cwd.is_none());
            assert!(request.envs.is_empty());
            success()
        }
        Some(script) if script.ends_with("https://www.google.com/") => Ok(SandboxProcessOutput {
            exit_code: Some(28),
            exited: true,
            ..SandboxProcessOutput::default()
        }),
        script => panic!("unexpected conformance process script: {script:?}"),
    }
}

fn success() -> Result<SandboxProcessOutput> {
    Ok(SandboxProcessOutput {
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
        ProcessStreamEvent::Exited {
            exit_code: 0,
            exited: true,
        },
        ProcessStreamEvent::Outcome(ProcessStreamOutcome::Completed),
    ])))
}
