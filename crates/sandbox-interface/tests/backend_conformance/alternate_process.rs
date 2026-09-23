//! Alternate-backend bounded process behavior.

use std::collections::BTreeMap;

use sandbox_interface::{BackendRunProcessRequest, Result, SandboxProcessOutput};

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
