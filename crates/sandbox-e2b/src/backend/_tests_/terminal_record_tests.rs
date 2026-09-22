//! Versioning and process-identity tests for trusted terminal records.

use sandbox_interface::{OperationId, TerminalId, TerminalState};

use crate::process::ProcessInfo;

use super::decode;

#[test]
fn current_record_recovers_exit_and_ignores_unrelated_pid_reuse() {
    let terminal_id = TerminalId::new();
    let operation_id = OperationId::new();
    let record = decode(&record_bytes(
        "sandbox-e2b-terminal-identity-v1",
        terminal_id,
        operation_id,
        "",
    ))
    .expect("current terminal identity record");

    record
        .ensure_request(terminal_id, operation_id)
        .expect("matching create request");
    assert_eq!(
        record.state(&[]).expect("exited terminal state"),
        TerminalState::Exited
    );
    assert_eq!(
        record
            .state(&[ProcessInfo {
                pid: 42,
                tag: Some("unrelated".to_owned()),
            }])
            .expect("PID reuse has a different tag"),
        TerminalState::Exited
    );
}

#[test]
fn unknown_or_extended_record_versions_fail_closed() {
    let terminal_id = TerminalId::new();
    let operation_id = OperationId::new();
    let unknown = record_bytes(
        "sandbox-e2b-terminal-identity-v2",
        terminal_id,
        operation_id,
        "",
    );
    let extended = record_bytes(
        "sandbox-e2b-terminal-identity-v1",
        terminal_id,
        operation_id,
        ",\"unexpected\":true",
    );

    assert!(decode(&unknown).is_err());
    assert!(decode(&extended).is_err());
}

fn record_bytes(
    schema: &str,
    terminal_id: TerminalId,
    operation_id: OperationId,
    extension: &str,
) -> Vec<u8> {
    format!(
        concat!(
            "{{\"schema\":\"{}\",\"pid\":42,",
            "\"terminal_id\":\"{}\",\"operation_id\":\"{}\",",
            "\"tag\":\"sandbox-terminal-{}\"{}}}\n"
        ),
        schema, terminal_id, operation_id, terminal_id, extension,
    )
    .into_bytes()
}
