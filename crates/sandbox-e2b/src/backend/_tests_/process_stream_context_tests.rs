//! Streaming context validation precedes provider access and other request bounds.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use futures_util::{StreamExt, stream};
use sandbox_interface::{
    BackendStreamProcessRequest, Error, PROCESS_RUN_MAX_STREAM_BYTES, ProcessRunContextError,
    ProcessStreamEvent, ProcessStreamOutcome, ProviderRef, SandboxBackend,
};
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn invalid_cwd_is_rejected_before_other_validation_or_provider_access() {
    let mut request = request();
    request.cwd = Some("relative-secret".to_owned());
    request.command.clear();
    request.stdout_limit = PROCESS_RUN_MAX_STREAM_BYTES + 1;
    request.stderr_limit = PROCESS_RUN_MAX_STREAM_BYTES + 1;
    request.deadline = Duration::MAX;
    request.idle_timeout = Duration::ZERO;

    let error = rejected(request).await;

    assert!(matches!(
        error,
        Error::InvalidProcessRunContext {
            reason: ProcessRunContextError::InvalidWorkingDirectory
        }
    ));
    assert!(!error.to_string().contains("relative-secret"));
}

#[tokio::test]
async fn template_owned_name_is_rejected_without_provider_access() {
    let mut request = request();
    request
        .envs
        .insert("PATH".to_owned(), "secret-path".to_owned());

    let error = rejected(request).await;

    assert!(matches!(
        error,
        Error::InvalidProcessRunContext {
            reason: ProcessRunContextError::TemplateOwnedEnvironmentName
        }
    ));
    assert!(!error.to_string().contains("secret-path"));
}

#[tokio::test]
async fn valid_context_is_forwarded_to_the_stream_transport_exactly() {
    let control = Arc::new(Unimock::new(
        E2bControlApiMock::connect_sandbox_with_timeout
            .next_call(matching!("provider", 600))
            .returns(Ok(ControlSandboxAccess {
                sandbox_id: "provider".to_owned(),
                domain: "untrusted.example".to_owned(),
                envd_access_token: "access-token".to_owned(),
                traffic_access_token: Some("traffic-token".to_owned()),
            })),
    ));
    let processes = Arc::new(Unimock::new(
        ProcessTransportMock::stream_process
            .next_call(matching!(_, _))
            .answers(&|_, connection, command| {
                assert_eq!(connection.sandbox_domain(), "e2b.app");
                assert_eq!(command.cwd.as_deref(), Some("/workspace/repo"));
                assert_eq!(
                    command.envs,
                    BTreeMap::from([(
                        "SANDBOX_PROBE".to_owned(),
                        "adapter-stream-value".to_owned()
                    )])
                );
                Ok(Box::pin(stream::iter([
                    ProcessStreamEvent::Started { pid: 17 },
                    ProcessStreamEvent::Stdout(b"adapter-stream-value\r\n\x1b[31m\xff".to_vec()),
                    ProcessStreamEvent::Stderr(b"adapter-stream-value".to_vec()),
                    ProcessStreamEvent::Exited {
                        exit_code: 0,
                        exited: true,
                    },
                    ProcessStreamEvent::Outcome(ProcessStreamOutcome::Completed),
                ])))
            }),
    ));
    let backend = E2bSandboxBackend::with_transports(config(), control, processes);
    let mut request = request();
    request.cwd = Some("/workspace/repo".to_owned());
    request.envs = BTreeMap::from([(
        "SANDBOX_PROBE".to_owned(),
        "adapter-stream-value".to_owned(),
    )]);

    let events = backend
        .stream_process(request)
        .await
        .expect("valid stream")
        .collect::<Vec<_>>()
        .await;

    assert_eq!(
        events[1],
        ProcessStreamEvent::Stdout(b"adapter-stream-value\r\n\x1b[31m\xff".to_vec())
    );
    assert_eq!(
        events[2],
        ProcessStreamEvent::Stderr(b"adapter-stream-value".to_vec())
    );
    assert_eq!(
        events.last(),
        Some(&ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::Completed
        ))
    );
}

async fn rejected(request: BackendStreamProcessRequest) -> Error {
    let backend = E2bSandboxBackend::with_transports(
        config(),
        Arc::new(Unimock::new(())),
        Arc::new(Unimock::new(())),
    );
    match backend.stream_process(request).await {
        Err(error) => error,
        Ok(_) => panic!("invalid streaming context should fail"),
    }
}

fn request() -> BackendStreamProcessRequest {
    BackendStreamProcessRequest {
        sandbox_provider_ref: ProviderRef::new("provider"),
        command: "/bin/true".to_owned(),
        args: Vec::new(),
        cwd: None,
        envs: BTreeMap::new(),
        stdout_limit: 4096,
        stderr_limit: 4096,
        deadline: Duration::from_secs(30),
        idle_timeout: Duration::from_secs(1),
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
