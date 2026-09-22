//! E2B terminal provider identity fencing tests.

use std::{collections::HashMap, sync::Arc, time::Duration};

use sandbox_interface::{
    BackendInputRequest, BackendOutputRequest, Error, ProviderRef, ResourceKind, SandboxBackend,
    TerminalId,
};
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, ProcessInfo,
    ProcessRunOutput, ProcessSelector, ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn stale_pid_identity_cannot_target_a_differently_tagged_process() {
    let terminal_id = TerminalId::new();
    let provider_ref = ProviderRef::new(format!("e2b-pty-v1:41:{terminal_id}"));
    let wrong_tag = format!("sandbox-terminal-{}", TerminalId::new());
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .each_call(matching!("sandbox"))
            .answers_arc(Arc::new(|_, _| Ok(access()))),
    );
    let processes = Unimock::new((
        ProcessTransportMock::list
            .each_call(matching!(_))
            .answers_arc(Arc::new(move |_, connection| {
                assert_eq!(connection.user(), Some("root"));
                Ok(vec![ProcessInfo {
                    pid: 41,
                    tag: Some(wrong_tag.clone()),
                }])
            })),
        ProcessTransportMock::read_regular_file
            .next_call(matching!(_, _))
            .returns(Err(Error::NotFound {
                resource: ResourceKind::File,
            })),
        ProcessTransportMock::kill
            .next_call(matching!(_, _))
            .answers(&|_, connection, _| {
                assert_eq!(connection.user(), Some("root"));
                Ok(())
            }),
        cleanup_success(),
    ));
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(processes));

    assert_not_found(
        backend
            .inspect_terminal(ProviderRef::new("sandbox"), provider_ref.clone())
            .await,
    );
    assert_not_found(
        backend
            .read_terminal(BackendOutputRequest {
                sandbox_provider_ref: ProviderRef::new("sandbox"),
                terminal_provider_ref: provider_ref.clone(),
                provider_log_path: format!("/var/lib/sandbox-e2b/terminals/{terminal_id}.log"),
                offset: 0,
                max_bytes: 1024,
                provider_log_limit: 2048,
                wait: Duration::ZERO,
            })
            .await,
    );
    assert_not_found(
        backend
            .write_terminal(BackendInputRequest {
                sandbox_provider_ref: ProviderRef::new("sandbox"),
                terminal_provider_ref: provider_ref.clone(),
                input: b"unsafe".to_vec(),
            })
            .await,
    );
    backend
        .close_terminal(ProviderRef::new("sandbox"), provider_ref)
        .await
        .expect("closing an absent tag is idempotent");
}

#[tokio::test]
async fn terminal_mutations_use_the_unique_tag_as_the_atomic_selector() {
    let terminal_id = TerminalId::new();
    let terminal_ref = ProviderRef::new(format!("e2b-pty-v1:41:{terminal_id}"));
    let expected_tag = format!("sandbox-terminal-{terminal_id}");
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .each_call(matching!("sandbox"))
            .answers_arc(Arc::new(|_, _| Ok(access()))),
    );
    let write_tag = expected_tag.clone();
    let close_tag = expected_tag;
    let processes = Unimock::new((
        ProcessTransportMock::list
            .next_call(matching!(_))
            .returns(Ok(vec![ProcessInfo {
                pid: 41,
                tag: Some(write_tag.clone()),
            }])),
        ProcessTransportMock::send_input
            .next_call(matching!(_, _, _))
            .answers_arc(Arc::new(move |_, connection, selector, input| {
                assert_eq!(connection.user(), Some("root"));
                assert_eq!(selector, ProcessSelector::Tag(write_tag.clone()));
                assert_eq!(input, b"safe");
                Ok(())
            })),
        ProcessTransportMock::kill
            .next_call(matching!(_, _))
            .answers_arc(Arc::new(move |_, connection, selector| {
                assert_eq!(connection.user(), Some("root"));
                assert_eq!(selector, ProcessSelector::Tag(close_tag.clone()));
                Ok(())
            })),
        cleanup_success(),
    ));
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(processes));

    backend
        .write_terminal(BackendInputRequest {
            sandbox_provider_ref: ProviderRef::new("sandbox"),
            terminal_provider_ref: terminal_ref.clone(),
            input: b"safe".to_vec(),
        })
        .await
        .expect("tag-addressed input");
    backend
        .close_terminal(ProviderRef::new("sandbox"), terminal_ref)
        .await
        .expect("tag-addressed close");
}

fn cleanup_success() -> impl unimock::Clause {
    ProcessTransportMock::run
        .next_call(matching!(_, _))
        .returns(Ok(ProcessRunOutput {
            bytes: Vec::new(),
            exit_code: Some(0),
            exited: true,
            output_truncated: false,
        }))
}

fn assert_not_found<T>(result: sandbox_interface::Result<T>) {
    assert!(matches!(
        result,
        Err(Error::NotFound {
            resource: ResourceKind::Terminal
        })
    ));
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
