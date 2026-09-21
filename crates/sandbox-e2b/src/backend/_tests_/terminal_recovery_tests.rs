use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{
    BackendTerminalCreateRequest, OperationId, ProviderRef, SandboxBackend, TerminalId,
};
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, E2bRuntimeConventions,
    ProcessInfo, ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn terminal_recovery_finds_the_tagged_process_without_restarting_it() {
    let terminal_id = TerminalId::new();
    let tag = format!("tenant-terminal-{terminal_id}");
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("sandbox"))
            .returns(Ok(ControlSandboxAccess {
                sandbox_id: "sandbox".to_owned(),
                domain: "e2b.app".to_owned(),
                envd_access_token: "call-local-token".to_owned(),
                traffic_access_token: "traffic-token".to_owned(),
            })),
    );
    let processes = Unimock::new(
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(vec![ProcessInfo {
                pid: 42,
                tag: Some(tag),
            }])),
    );
    let config = config().with_runtime_conventions(
        E2bRuntimeConventions::new("tenant", "tenant-terminal-", "/opt/tenant/screen-helper")
            .unwrap(),
    );
    let backend =
        E2bSandboxBackend::with_transports(config, Arc::new(control), Arc::new(processes));

    let terminal = backend
        .recover_terminal_create(BackendTerminalCreateRequest {
            terminal_id,
            operation_id: OperationId::new(),
            sandbox_provider_ref: ProviderRef::new("sandbox"),
            cwd: Some("/workspace".to_owned()),
            provider_log_limit: 2 * 1024 * 1024,
        })
        .await
        .expect("tagged process recovery should succeed")
        .expect("tagged process should recover");

    assert_eq!(
        terminal.provider_ref.as_str(),
        format!("e2b-pty-v1:42:{terminal_id}")
    );
    assert!(
        terminal
            .provider_log_path
            .ends_with(&format!("/{terminal_id}.log"))
    );
}

#[tokio::test]
async fn terminal_recovery_does_not_allocate_when_the_tag_is_absent() {
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("sandbox"))
            .returns(Ok(ControlSandboxAccess {
                sandbox_id: "sandbox".to_owned(),
                domain: "e2b.app".to_owned(),
                envd_access_token: "call-local-token".to_owned(),
                traffic_access_token: "traffic-token".to_owned(),
            })),
    );
    let processes = Unimock::new(
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(processes));

    let terminal = backend
        .recover_terminal_create(BackendTerminalCreateRequest {
            terminal_id: TerminalId::new(),
            operation_id: OperationId::new(),
            sandbox_provider_ref: ProviderRef::new("sandbox"),
            cwd: None,
            provider_log_limit: 2 * 1024 * 1024,
        })
        .await
        .expect("absent terminal recovery should succeed");

    assert!(terminal.is_none());
}

#[tokio::test]
async fn restored_cleanup_unmounts_and_removes_drive_credentials() {
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("sandbox"))
            .returns(Ok(ControlSandboxAccess {
                sandbox_id: "sandbox".to_owned(),
                domain: "e2b.app".to_owned(),
                envd_access_token: "call-local-token".to_owned(),
                traffic_access_token: "traffic-token".to_owned(),
            })),
    );
    let processes = Unimock::new((
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .answers(&|_, _, command| {
                assert_eq!(command.command, "/bin/sh");
                assert!(command.args[1].contains("fusermount3 -u /drives/me"));
                assert!(command.args[1].contains("[s]andbox-drive-"));
                assert!(command.args[1].contains("/tmp/sandbox-drive"));
                Ok(crate::ProcessRunOutput {
                    bytes: Vec::new(),
                    exit_code: Some(0),
                    exited: true,
                    output_truncated: false,
                })
            }),
    ));
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(processes));

    backend
        .clean_restored_terminals(ProviderRef::new("sandbox"))
        .await
        .expect("restored cleanup should succeed");
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
