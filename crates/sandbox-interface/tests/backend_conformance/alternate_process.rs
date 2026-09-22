//! Alternate-backend bounded process behavior.

use std::collections::BTreeMap;

use sandbox_interface::{BackendRunProcessRequest, Result, SandboxProcessOutput};

pub(super) fn run(request: BackendRunProcessRequest) -> Result<SandboxProcessOutput> {
    assert_eq!(request.command, "/bin/sh");
    assert_eq!(request.args, ["-c", "pwd; printf %s \"$SANDBOX_PROBE\""]);
    assert_eq!(request.cwd.as_deref(), Some("/workspace"));
    assert_eq!(
        request.envs,
        BTreeMap::from([("SANDBOX_PROBE".to_owned(), "environment-map".to_owned())])
    );
    Ok(SandboxProcessOutput {
        stdout: b"/workspace\nenvironment-map".to_vec(),
        stderr: Vec::new(),
        exit_code: Some(0),
        exited: true,
        ..SandboxProcessOutput::default()
    })
}
