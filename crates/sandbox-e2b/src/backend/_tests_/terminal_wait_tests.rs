//! Durable terminal long-poll behavior tests.

use std::{collections::HashMap, sync::Arc, time::Duration};

use sandbox_interface::{
    BackendOutputRequest, BackendTerminalCreateRequest, Error, OperationId, ProviderRef,
    ResourceKind, SandboxBackend, TerminalId, TerminalState,
};
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, ProcessFileChunk,
    ProcessInfo, ProcessRunOutput, ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn terminal_create_and_durable_read_map_process_state() {
    let terminal_id = TerminalId::new();
    let control = Unimock::new((
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("source"))
            .returns(Ok(access("source"))),
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("source"))
            .returns(Ok(access("source"))),
    ));
    let processes = Unimock::new((
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .answers(&|_, connection, _| {
                assert_eq!(connection.user(), Some("root"));
                Ok(successful_run())
            }),
        ProcessTransportMock::list
            .next_call(matching!(_))
            .answers(&|_, connection| {
                assert_eq!(connection.user(), Some("root"));
                Ok(Vec::new())
            }),
        missing_identity(),
        ProcessTransportMock::start_pty
            .next_call(matching!(_, _))
            .answers_arc(Arc::new(move |_, connection, request| {
                assert_eq!(connection.user(), Some("root"));
                assert_eq!(request.workload_user, "user");
                Ok(process(terminal_id))
            })),
        ProcessTransportMock::list
            .next_call(matching!(_))
            .answers_arc(Arc::new(move |_, connection| {
                assert_eq!(connection.user(), Some("root"));
                Ok(vec![process(terminal_id)])
            })),
        missing_identity(),
        ProcessTransportMock::read_regular_file
            .next_call(matching!(_, _))
            .answers(&|_, connection, _| {
                assert_eq!(connection.user(), Some("root"));
                Ok(ProcessFileChunk {
                    bytes: b"output".to_vec(),
                    total_size: 64,
                })
            }),
    ));
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(processes));

    let terminal = backend
        .create_terminal(BackendTerminalCreateRequest {
            terminal_id,
            operation_id: OperationId::new(),
            sandbox_provider_ref: ProviderRef::new("source"),
            cwd: Some("/workspace".to_owned()),
            provider_log_limit: 64,
        })
        .await
        .expect("terminal create");
    let output = backend
        .read_terminal(BackendOutputRequest {
            sandbox_provider_ref: ProviderRef::new("source"),
            terminal_provider_ref: terminal.provider_ref.clone(),
            provider_log_path: terminal.provider_log_path.clone(),
            offset: 5,
            max_bytes: 16,
            provider_log_limit: 64,
            wait: Duration::ZERO,
        })
        .await
        .expect("durable read");

    assert_eq!(terminal.provider_ref, provider_ref(terminal_id));
    assert!(
        terminal
            .provider_log_path
            .contains(&terminal_id.to_string())
    );
    assert_eq!(output.bytes, b"output");
    assert_eq!(output.next_offset, 11);
    assert_eq!(output.state, TerminalState::Ready);
    assert!(output.overflowed);
}

#[tokio::test]
async fn terminal_read_waits_for_delayed_durable_output() {
    let terminal_id = TerminalId::new();
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("sandbox"))
            .returns(Ok(access("sandbox"))),
    );
    let processes = Unimock::new((
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(vec![process(terminal_id)])),
        missing_identity(),
        ProcessTransportMock::read_regular_file
            .next_call(matching!(_, _))
            .returns(Ok(ProcessFileChunk {
                bytes: Vec::new(),
                total_size: 0,
            })),
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(vec![process(terminal_id)])),
        missing_identity(),
        ProcessTransportMock::read_regular_file
            .next_call(matching!(_, _))
            .returns(Ok(ProcessFileChunk {
                bytes: b"delayed".to_vec(),
                total_size: 7,
            })),
    ));
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(processes));

    let output = backend
        .read_terminal(BackendOutputRequest {
            sandbox_provider_ref: ProviderRef::new("sandbox"),
            terminal_provider_ref: provider_ref(terminal_id),
            provider_log_path: format!("/var/lib/sandbox-e2b/terminals/{terminal_id}.log"),
            offset: 0,
            max_bytes: 1024,
            provider_log_limit: 1024,
            wait: Duration::from_millis(1),
        })
        .await
        .expect("delayed output should be observed within the wait");

    assert_eq!(output.bytes, b"delayed");
    assert_eq!(output.next_offset, 7);
}

#[tokio::test]
async fn terminal_read_retries_output_appended_after_an_empty_chunk() {
    let terminal_id = TerminalId::new();
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("sandbox"))
            .returns(Ok(access("sandbox"))),
    );
    let processes = Unimock::new((
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(vec![process(terminal_id)])),
        missing_identity(),
        ProcessTransportMock::read_regular_file
            .next_call(matching!(_, _))
            .returns(Ok(ProcessFileChunk {
                bytes: Vec::new(),
                total_size: 7,
            })),
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(vec![process(terminal_id)])),
        missing_identity(),
        ProcessTransportMock::read_regular_file
            .next_call(matching!(_, _))
            .returns(Ok(ProcessFileChunk {
                bytes: b"appended".to_vec(),
                total_size: 8,
            })),
    ));
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(processes));

    let output = backend
        .read_terminal(BackendOutputRequest {
            sandbox_provider_ref: ProviderRef::new("sandbox"),
            terminal_provider_ref: provider_ref(terminal_id),
            provider_log_path: format!("/var/lib/sandbox-e2b/terminals/{terminal_id}.log"),
            offset: 0,
            max_bytes: 1024,
            provider_log_limit: 1024,
            wait: Duration::ZERO,
        })
        .await
        .expect("post-read growth should be retried immediately");

    assert_eq!(output.bytes, b"appended");
    assert_eq!(output.next_offset, 8);
    assert_eq!(output.total_size, 8);
}

#[tokio::test]
async fn terminal_read_treats_a_missing_log_as_empty_while_the_process_is_alive() {
    let terminal_id = TerminalId::new();
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("sandbox"))
            .returns(Ok(access("sandbox"))),
    );
    let processes = Unimock::new((
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(vec![process(terminal_id)])),
        missing_identity(),
        ProcessTransportMock::read_regular_file
            .next_call(matching!(_, _))
            .returns(Err(Error::NotFound {
                resource: ResourceKind::Terminal,
            })),
    ));
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(processes));

    let output = backend
        .read_terminal(BackendOutputRequest {
            sandbox_provider_ref: ProviderRef::new("sandbox"),
            terminal_provider_ref: provider_ref(terminal_id),
            provider_log_path: format!("/var/lib/sandbox-e2b/terminals/{terminal_id}.log"),
            offset: 0,
            max_bytes: 1024,
            provider_log_limit: 1024,
            wait: Duration::ZERO,
        })
        .await
        .expect("a live PTY should outlast provider-log creation latency");

    assert!(output.bytes.is_empty());
    assert_eq!(output.state, TerminalState::Ready);
}

fn process(terminal_id: TerminalId) -> ProcessInfo {
    ProcessInfo {
        pid: 41,
        tag: Some(format!("sandbox-terminal-{terminal_id}")),
    }
}

fn provider_ref(terminal_id: TerminalId) -> ProviderRef {
    ProviderRef::new(format!("e2b-pty-v1:41:{terminal_id}"))
}

fn missing_identity() -> impl unimock::Clause {
    ProcessTransportMock::read_regular_file
        .next_call(matching!(_, _))
        .returns(Err(Error::NotFound {
            resource: ResourceKind::File,
        }))
}

fn access(sandbox_id: &str) -> ControlSandboxAccess {
    ControlSandboxAccess {
        sandbox_id: sandbox_id.to_owned(),
        domain: "e2b.app".to_owned(),
        envd_access_token: "call-local-token".to_owned(),
        traffic_access_token: Some("traffic-token".to_owned()),
    }
}

fn successful_run() -> ProcessRunOutput {
    ProcessRunOutput {
        bytes: Vec::new(),
        exit_code: Some(0),
        exited: true,
        output_truncated: false,
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
