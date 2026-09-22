//! Alternate-backend bounded process behavior.

use sandbox_interface::{BackendRunProcessRequest, Result, SandboxProcessOutput};

pub(super) fn run(request: BackendRunProcessRequest) -> Result<SandboxProcessOutput> {
    assert_eq!(request.command, "/bin/sh");
    assert_eq!(request.args.first().map(String::as_str), Some("-c"));
    match request.args.get(1).map(String::as_str) {
        Some("printf '%s' 'argv-direct'; printf '%s' 'separate-stderr' >&2") => {
            Ok(SandboxProcessOutput {
                stdout: b"argv-direct".to_vec(),
                stderr: b"separate-stderr".to_vec(),
                exit_code: Some(0),
                exited: true,
                ..SandboxProcessOutput::default()
            })
        }
        Some(script) if script.ends_with("https://example.com/") => success(),
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
