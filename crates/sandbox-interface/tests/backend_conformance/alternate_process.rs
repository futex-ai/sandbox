//! Alternate-backend bounded process behavior.

use sandbox_interface::{BackendRunProcessRequest, Result, SandboxProcessOutput};

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
