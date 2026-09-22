//! Internal process command diagnostics omit text even without environment values.

use std::{collections::BTreeMap, time::Duration};

use crate::process::types::{ProcessCommand, ProcessOutputCapture, SplitProcessCommand};

#[test]
fn command_debug_reports_only_approved_metadata() {
    let envs = BTreeMap::from([("internal-secret".to_owned(), "internal-secret".to_owned())]);
    let combined = ProcessCommand {
        command: "internal-secret".to_owned(),
        args: vec!["internal-secret".to_owned()],
        cwd: Some("/internal-secret".to_owned()),
        envs: envs.clone(),
        output_capture: ProcessOutputCapture::HardLimit { max_bytes: 1 },
        timeout: Duration::from_secs(1),
        read_only: false,
    };
    let split = SplitProcessCommand {
        command: combined.command.clone(),
        args: combined.args.clone(),
        cwd: combined.cwd.clone(),
        envs,
        stdout_limit: 1,
        stderr_limit: 1,
        deadline: Duration::from_secs(1),
    };

    assert_eq!(
        format!("{combined:?}"),
        "ProcessCommand { arg_count: 1, has_cwd: true, \
        env_count: 1, output_capture: HardLimit { max_bytes: 1 }, timeout: 1s, read_only: false }"
    );
    assert_eq!(
        format!("{split:?}"),
        "SplitProcessCommand { arg_count: 1, has_cwd: true, \
        env_count: 1, stdout_limit: 1, stderr_limit: 1, deadline: 1s }"
    );
    for command in [
        combined.clone(),
        ProcessCommand {
            envs: BTreeMap::new(),
            ..combined
        },
    ] {
        for debug in [format!("{command:?}"), format!("{command:#?}")] {
            assert!(!debug.contains("internal-secret"));
            assert!(!debug.contains("command:"));
            assert!(!debug.contains("args:"));
            assert!(!debug.contains("envs:"));
        }
    }
    assert!(!format!("{split:#?}").contains("internal-secret"));
}
