//! Exact execution context encoding for direct commands and PTYs.

use std::{collections::BTreeMap, time::Duration};

use sandbox_interface::{OperationId, TerminalId};

use crate::{ProcessCommand, ProcessOutputCapture, ProcessPtyRequest};

use super::{argv_start, command_start, encode, pty_start};

#[test]
fn argv_start_encodes_working_directory_and_environment() {
    let body = encode(&argv_start(
        "/bin/sh".to_owned(),
        vec!["-c".to_owned(), "pwd".to_owned()],
        Some("/workspace".to_owned()),
        BTreeMap::from([("SANDBOX_PROBE".to_owned(), "wire-value".to_owned())]),
    ))
    .expect("direct process body");
    let body: serde_json::Value = serde_json::from_slice(&body).expect("direct process JSON");

    assert_eq!(body["process"]["cmd"], "/bin/sh");
    assert_eq!(body["process"]["args"], serde_json::json!(["-c", "pwd"]));
    assert_eq!(body["process"]["cwd"], "/workspace");
    assert_eq!(
        body["process"]["envs"],
        serde_json::json!({"SANDBOX_PROBE": "wire-value"})
    );
    assert_eq!(body["stdin"], false);
    assert!(body.get("pty").is_none());
}

#[test]
fn combined_command_start_uses_the_same_execution_context() {
    let body = encode(&command_start(ProcessCommand {
        command: "/bin/true".to_owned(),
        args: Vec::new(),
        cwd: Some("/workspace/repo".to_owned()),
        envs: BTreeMap::from([("SAFE_NAME".to_owned(), "combined-value".to_owned())]),
        output_capture: ProcessOutputCapture::HardLimit { max_bytes: 1 },
        timeout: Duration::from_secs(1),
        read_only: false,
    }))
    .expect("combined process body");
    let body: serde_json::Value = serde_json::from_slice(&body).expect("combined process JSON");

    assert_eq!(body["process"]["cwd"], "/workspace/repo");
    assert_eq!(
        body["process"]["envs"],
        serde_json::json!({"SAFE_NAME": "combined-value"})
    );
}

#[test]
fn pty_start_keeps_fixed_locale_environment_and_working_directory() {
    let body = encode(&pty_start(ProcessPtyRequest {
        tag: "sandbox-terminal".to_owned(),
        log_path: "/tmp/sandbox.log".to_owned(),
        identity_path: "/tmp/sandbox.identity.json".to_owned(),
        log_limit: 1024,
        workload_user: "user".to_owned(),
        terminal_id: TerminalId::new(),
        operation_id: OperationId::new(),
        cwd: Some("/workspace".to_owned()),
    }))
    .expect("PTY body");
    let body: serde_json::Value = serde_json::from_slice(&body).expect("PTY JSON");

    assert_eq!(body["process"]["cwd"], "/workspace");
    assert_eq!(
        body["process"]["envs"],
        serde_json::json!({
            "LANG": "C.UTF-8",
            "LC_ALL": "C.UTF-8",
            "TERM": "xterm-256color"
        })
    );
}
