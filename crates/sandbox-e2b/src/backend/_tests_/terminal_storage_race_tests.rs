//! Descriptor-bound terminal transcript read regressions.

use std::{collections::HashMap, sync::Arc, time::Duration};

use sandbox_interface::{
    BackendOutputRequest, Error, OperationId, ProviderRef, SandboxBackend, TerminalId,
    TerminalState,
};
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, ProcessFileChunk,
    ProcessInfo, ProcessRegularFileRequest, ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn terminal_output_uses_a_non_following_root_relative_read() {
    let terminal_id = TerminalId::new();
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("sandbox"))
            .returns(Ok(access())),
    );
    let processes = Unimock::new((
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(vec![process(terminal_id)])),
        ProcessTransportMock::read_regular_file
            .next_call(matching!(_, _))
            .answers_arc(Arc::new(
                move |_, connection, request: ProcessRegularFileRequest| {
                    assert_eq!(connection.user(), Some("root"));
                    assert_eq!(request.root, "/var/lib/sandbox-e2b/terminals");
                    assert_eq!(request.path, format!("{terminal_id}.log"));
                    Ok(ProcessFileChunk {
                        bytes: b"captured".to_vec(),
                        total_size: 8,
                    })
                },
            )),
    ));
    let backend = backend(control, processes);

    let output = backend
        .read_terminal(request(terminal_id, expected_log_path(terminal_id)))
        .await
        .expect("descriptor-relative transcript read");

    assert_eq!(output.bytes, b"captured");
    assert_eq!(output.state, TerminalState::Ready);
}

#[tokio::test]
async fn terminal_output_rejects_a_substituted_log_path_before_provider_access() {
    let terminal_id = TerminalId::new();
    let backend = backend(Unimock::new(()), Unimock::new(()));

    let result = backend
        .read_terminal(request(terminal_id, "/tmp/attacker.log".to_owned()))
        .await;

    assert!(matches!(
        result,
        Err(Error::InvalidFilePath {
            field: "provider_log_path"
        })
    ));
}

#[tokio::test]
async fn exited_terminal_output_survives_unrelated_pid_reuse() {
    let terminal_id = TerminalId::new();
    let operation_id = OperationId::new();
    let identity = identity_record(terminal_id, operation_id);
    let identity_size = identity.len() as u64;
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("sandbox"))
            .returns(Ok(access())),
    );
    let processes = Unimock::new((
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(vec![ProcessInfo {
                pid: 41,
                tag: Some("unrelated-process".to_owned()),
            }])),
        ProcessTransportMock::read_regular_file
            .next_call(matching!(_, _))
            .answers_arc(Arc::new(move |_, _, request: ProcessRegularFileRequest| {
                assert_eq!(request.path, format!("{terminal_id}.identity.json"));
                Ok(ProcessFileChunk {
                    bytes: identity.clone(),
                    total_size: identity_size,
                })
            })),
        ProcessTransportMock::read_regular_file
            .next_call(matching!(_, _))
            .answers_arc(Arc::new(move |_, _, request: ProcessRegularFileRequest| {
                assert_eq!(request.path, format!("{terminal_id}.log"));
                Ok(ProcessFileChunk {
                    bytes: b"final output".to_vec(),
                    total_size: 12,
                })
            })),
    ));
    let backend = backend(control, processes);

    let output = backend
        .read_terminal(request(terminal_id, expected_log_path(terminal_id)))
        .await
        .expect("durable transcript should survive unrelated PID reuse");

    assert_eq!(output.bytes, b"final output");
    assert_eq!(output.state, TerminalState::Exited);
}

fn request(terminal_id: TerminalId, provider_log_path: String) -> BackendOutputRequest {
    BackendOutputRequest {
        sandbox_provider_ref: ProviderRef::new("sandbox"),
        terminal_provider_ref: ProviderRef::new(format!("e2b-pty-v1:41:{terminal_id}")),
        provider_log_path,
        offset: 0,
        max_bytes: 1024,
        provider_log_limit: 1024,
        wait: Duration::ZERO,
    }
}

fn expected_log_path(terminal_id: TerminalId) -> String {
    format!("/var/lib/sandbox-e2b/terminals/{terminal_id}.log")
}

fn process(terminal_id: TerminalId) -> ProcessInfo {
    ProcessInfo {
        pid: 41,
        tag: Some(format!("sandbox-terminal-{terminal_id}")),
    }
}

fn identity_record(terminal_id: TerminalId, operation_id: OperationId) -> Vec<u8> {
    format!(
        concat!(
            "{{\"schema\":\"sandbox-e2b-terminal-identity-v1\",",
            "\"pid\":41,\"terminal_id\":\"{}\",",
            "\"operation_id\":\"{}\",",
            "\"tag\":\"sandbox-terminal-{}\"}}\n"
        ),
        terminal_id, operation_id, terminal_id,
    )
    .into_bytes()
}

fn access() -> ControlSandboxAccess {
    ControlSandboxAccess {
        sandbox_id: "sandbox".to_owned(),
        domain: "e2b.app".to_owned(),
        envd_access_token: "token".to_owned(),
        traffic_access_token: Some("traffic-token".to_owned()),
    }
}

fn backend(control: Unimock, processes: Unimock) -> E2bSandboxBackend {
    E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(processes))
}

fn config() -> E2bAdapterConfig {
    E2bAdapterConfig::new(
        "configured-e2b",
        "https://api.e2b.app",
        "api-key",
        HashMap::from([(
            "general".to_owned(),
            E2bProfile {
                template: "base".to_owned(),
                allow_public_egress: false,
                denied_destinations: vec!["203.0.113.10/32".to_owned()],
            },
        )]),
        600,
    )
    .expect("valid config")
}
