//! Internal process command debug-redaction tests.

use std::{collections::BTreeMap, time::Duration};

use crate::process::types::{ProcessCommand, ProcessOutputCapture, SplitProcessCommand};

#[test]
fn command_debug_redacts_every_environment_value() {
    let envs = BTreeMap::from([("SANDBOX_TOKEN".to_owned(), "internal-secret".to_owned())]);
    let combined = ProcessCommand {
        command: "/bin/true".to_owned(),
        args: Vec::new(),
        cwd: None,
        envs: envs.clone(),
        output_capture: ProcessOutputCapture::HardLimit { max_bytes: 1 },
        timeout: Duration::from_secs(1),
        read_only: false,
    };
    let split = SplitProcessCommand {
        command: "/bin/true".to_owned(),
        args: Vec::new(),
        cwd: None,
        envs,
        stdout_limit: 1,
        stderr_limit: 1,
        deadline: Duration::from_secs(1),
    };

    for debug in [format!("{combined:?}"), format!("{split:?}")] {
        assert!(debug.contains("SANDBOX_TOKEN"));
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("internal-secret"));
    }
}
