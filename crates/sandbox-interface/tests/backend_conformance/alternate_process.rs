//! Alternate-backend bounded process behavior.

use sandbox_interface::{BackendRunProcessRequest, Result, SandboxProcessOutput};

pub(super) fn run(request: BackendRunProcessRequest) -> Result<SandboxProcessOutput> {
    assert_eq!(request.command, "conformance-command");
    assert_eq!(request.args, ["exact-argument"]);
    Ok(SandboxProcessOutput {
        stdout: b"argv-direct".to_vec(),
        stderr: b"separate-stderr".to_vec(),
        exit_code: Some(0),
        exited: true,
        ..SandboxProcessOutput::default()
    })
}
