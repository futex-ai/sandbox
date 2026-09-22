//! Lifecycle behavior after a durable terminal's provider process exits.

use std::{collections::HashMap, sync::Arc, time::Duration};

use sandbox_interface::{
    BackendInputRequest, BackendOutputRequest, Error, OperationId, ProviderRef, SandboxBackend,
    TerminalId, TerminalState,
};
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, ProcessFileChunk,
    ProcessRegularFileRequest, ProcessSelector, ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn terminal_input_rejects_an_exited_process_before_dispatch() {
    let terminal_id = TerminalId::new();
    let processes = Unimock::new(
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
    );
    let backend = backend(processes);

    let error = backend
        .write_terminal(BackendInputRequest {
            sandbox_provider_ref: ProviderRef::new("sandbox"),
            terminal_provider_ref: terminal_ref(terminal_id),
            input: b"must not be sent".to_vec(),
        })
        .await
        .expect_err("input to an exited terminal must be rejected");

    assert!(matches!(
        error,
        Error::TerminalStateConflict {
            state: TerminalState::Exited
        }
    ));
}

#[tokio::test]
async fn repeated_terminal_close_keeps_final_output_readable() {
    let terminal_id = TerminalId::new();
    let operation_id = OperationId::new();
    let expected_tag = format!("sandbox-terminal-{terminal_id}");
    let identity = identity_record(terminal_id, operation_id);
    let identity_size = identity.len() as u64;
    let processes = Unimock::new((
        close_call(expected_tag.clone()),
        close_call(expected_tag),
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
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
    let backend = backend(processes);
    let terminal_ref = terminal_ref(terminal_id);

    backend
        .close_terminal(ProviderRef::new("sandbox"), terminal_ref.clone())
        .await
        .expect("first close");
    backend
        .close_terminal(ProviderRef::new("sandbox"), terminal_ref.clone())
        .await
        .expect("repeated close");
    let output = backend
        .read_terminal(BackendOutputRequest {
            sandbox_provider_ref: ProviderRef::new("sandbox"),
            terminal_provider_ref: terminal_ref,
            provider_log_path: format!("/var/lib/sandbox-e2b/terminals/{terminal_id}.log"),
            offset: 0,
            max_bytes: 1024,
            provider_log_limit: 1024,
            wait: Duration::ZERO,
        })
        .await
        .expect("post-close final output");

    assert_eq!(output.bytes, b"final output");
    assert_eq!(output.state, TerminalState::Exited);
}

fn close_call(expected_tag: String) -> impl unimock::Clause {
    ProcessTransportMock::kill
        .next_call(matching!(_, _))
        .answers_arc(Arc::new(move |_, connection, selector| {
            assert_eq!(connection.user(), Some("root"));
            assert_eq!(selector, ProcessSelector::Tag(expected_tag.clone()));
            Ok(())
        }))
}

fn terminal_ref(terminal_id: TerminalId) -> ProviderRef {
    ProviderRef::new(format!("e2b-pty-v1:42:{terminal_id}"))
}

fn identity_record(terminal_id: TerminalId, operation_id: OperationId) -> Vec<u8> {
    format!(
        concat!(
            "{{\"schema\":\"sandbox-e2b-terminal-identity-v1\",",
            "\"pid\":42,\"terminal_id\":\"{}\",",
            "\"operation_id\":\"{}\",",
            "\"tag\":\"sandbox-terminal-{}\"}}\n"
        ),
        terminal_id, operation_id, terminal_id,
    )
    .into_bytes()
}

fn backend(processes: Unimock) -> E2bSandboxBackend {
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .each_call(matching!("sandbox"))
            .answers(&|_, _| Ok(access())),
    );
    E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(processes))
}

fn access() -> ControlSandboxAccess {
    ControlSandboxAccess {
        sandbox_id: "sandbox".to_owned(),
        domain: "e2b.app".to_owned(),
        envd_access_token: "call-local-token".to_owned(),
        traffic_access_token: Some("traffic-token".to_owned()),
    }
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
    .expect("valid adapter config")
}
