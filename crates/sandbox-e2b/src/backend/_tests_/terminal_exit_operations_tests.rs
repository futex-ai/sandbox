//! Lifecycle behavior after a durable terminal's provider process exits.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{
    BackendInputRequest, Error, ProviderRef, SandboxBackend, TerminalId, TerminalState,
};
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, ProcessRunOutput,
    ProcessSelector, ProcessTransportMock,
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
async fn terminal_close_removes_the_identity_record_idempotently() {
    let terminal_id = TerminalId::new();
    let expected_tag = format!("sandbox-terminal-{terminal_id}");
    let expected_path = format!("/var/lib/sandbox-e2b/terminals/{terminal_id}.identity.json");
    let processes = Unimock::new((
        close_call(expected_tag.clone()),
        identity_cleanup(expected_path.clone()),
        close_call(expected_tag),
        identity_cleanup(expected_path),
    ));
    let backend = backend(processes);
    let terminal_ref = terminal_ref(terminal_id);

    backend
        .close_terminal(ProviderRef::new("sandbox"), terminal_ref.clone())
        .await
        .expect("first close");
    backend
        .close_terminal(ProviderRef::new("sandbox"), terminal_ref)
        .await
        .expect("repeated close");
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

fn identity_cleanup(expected_path: String) -> impl unimock::Clause {
    ProcessTransportMock::run
        .next_call(matching!(_, _))
        .answers_arc(Arc::new(move |_, connection, command| {
            assert_eq!(connection.user(), Some("root"));
            assert_eq!(command.command, "/usr/bin/python3");
            assert!(
                command
                    .args
                    .iter()
                    .any(|argument| argument == &expected_path)
            );
            assert!(command.args[3].contains("os.O_NOFOLLOW"));
            Ok(ProcessRunOutput {
                bytes: Vec::new(),
                exit_code: Some(0),
                exited: true,
                output_truncated: false,
            })
        }))
}

fn terminal_ref(terminal_id: TerminalId) -> ProviderRef {
    ProviderRef::new(format!("e2b-pty-v1:42:{terminal_id}"))
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
        traffic_access_token: "traffic-token".to_owned(),
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
