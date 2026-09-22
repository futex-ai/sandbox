//! Recovery regressions for terminals whose shells exit during creation.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{
    BackendTerminalCreateRequest, Error, OperationId, ProviderRef, ResourceKind, SandboxBackend,
    TerminalId, TerminalState,
};
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, ProcessFileChunk,
    ProcessInfo, ProcessRunOutput, ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn terminal_create_initializes_storage_before_identity_lookup() {
    let request = terminal_request();
    let terminal_id = request.terminal_id;
    let processes = Unimock::new((
        successful_directory_creation(),
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
        missing_identity(),
        ProcessTransportMock::start_pty
            .next_call(matching!(_, _))
            .returns(Ok(process(terminal_id))),
    ));
    let backend = backend(processes);

    let terminal = backend
        .create_terminal(request)
        .await
        .expect("fresh terminal storage should initialize before recovery");

    assert_eq!(
        terminal.provider_ref,
        ProviderRef::new(format!("e2b-pty-v1:42:{terminal_id}"))
    );
}

#[tokio::test]
async fn acknowledged_terminal_start_recovers_after_an_immediate_exit() {
    let request = terminal_request();
    let terminal_id = request.terminal_id;
    let operation_id = request.operation_id;
    let processes = Unimock::new((
        successful_directory_creation(),
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
        missing_identity(),
        ProcessTransportMock::start_pty
            .next_call(matching!(_, _))
            .returns(Ok(process(terminal_id))),
        successful_directory_creation(),
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
        stored_identity(terminal_id, operation_id),
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
        stored_identity(terminal_id, operation_id),
    ));
    let backend = backend(processes);

    let created = backend
        .create_terminal(request.clone())
        .await
        .expect("provider acknowledged terminal start");
    let recovered = backend
        .recover_terminal_create(request)
        .await
        .expect("exited terminal recovery")
        .expect("durable terminal identity");
    let inspected = backend
        .inspect_terminal(ProviderRef::new("sandbox"), created.provider_ref.clone())
        .await
        .expect("exited terminal inspection");

    assert_eq!(created.provider_ref, recovered.provider_ref);
    assert_eq!(created.provider_ref, inspected.provider_ref);
    assert_eq!(recovered.state, TerminalState::Exited);
    assert_eq!(inspected.state, TerminalState::Exited);
}

#[tokio::test]
async fn ambiguous_terminal_start_recovers_after_an_immediate_exit() {
    let request = terminal_request();
    let terminal_id = request.terminal_id;
    let operation_id = request.operation_id;
    let processes = Unimock::new((
        successful_directory_creation(),
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
        missing_identity(),
        ProcessTransportMock::start_pty
            .next_call(matching!(_, _))
            .returns(Err(Error::DeliveryUnknown)),
        successful_directory_creation(),
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
        stored_identity(terminal_id, operation_id),
    ));
    let backend = backend(processes);

    let error = backend
        .create_terminal(request.clone())
        .await
        .expect_err("interrupted start response must stay ambiguous");
    let recovered = backend
        .recover_terminal_create(request)
        .await
        .expect("ambiguous start recovery")
        .expect("durable terminal identity");

    assert!(matches!(error, Error::DeliveryUnknown));
    assert_eq!(
        recovered.provider_ref,
        ProviderRef::new(format!("e2b-pty-v1:42:{terminal_id}"))
    );
    assert_eq!(recovered.state, TerminalState::Exited);
}

fn missing_identity() -> impl unimock::Clause {
    ProcessTransportMock::read_regular_file
        .next_call(matching!(_, _))
        .returns(Err(Error::NotFound {
            resource: ResourceKind::File,
        }))
}

fn stored_identity(terminal_id: TerminalId, operation_id: OperationId) -> impl unimock::Clause {
    let bytes = identity_record(terminal_id, operation_id);
    let total_size = bytes.len() as u64;
    ProcessTransportMock::read_regular_file
        .next_call(matching!(_, _))
        .returns(Ok(ProcessFileChunk { bytes, total_size }))
}

fn successful_directory_creation() -> impl unimock::Clause {
    ProcessTransportMock::run
        .next_call(matching!(_, _))
        .returns(Ok(ProcessRunOutput {
            bytes: Vec::new(),
            exit_code: Some(0),
            exited: true,
            output_truncated: false,
        }))
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

fn terminal_request() -> BackendTerminalCreateRequest {
    BackendTerminalCreateRequest {
        terminal_id: TerminalId::new(),
        operation_id: OperationId::new(),
        sandbox_provider_ref: ProviderRef::new("sandbox"),
        cwd: None,
        provider_log_limit: 2 * 1024 * 1024,
    }
}

fn process(terminal_id: TerminalId) -> ProcessInfo {
    ProcessInfo {
        pid: 42,
        tag: Some(format!("sandbox-terminal-{terminal_id}")),
    }
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
