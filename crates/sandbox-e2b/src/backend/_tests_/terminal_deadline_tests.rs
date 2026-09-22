//! Provider-log deadline propagation regressions.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use sandbox_interface::{
    BackendOutputRequest, Error, ProviderRef, SandboxBackend, TerminalId, TerminalState,
};
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, ProcessFileChunk,
    ProcessInfo, ProcessRegularFileRequest, ProcessTransportMock,
};

use super::{configured::E2bSandboxBackend, terminal_identity::TerminalIdentity};

#[tokio::test]
async fn terminal_reads_pass_zero_and_max_wait_bounded_helper_deadlines() {
    let terminal_id = TerminalId::new();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .each_call(matching!("sandbox"))
            .answers(&|_, _| Ok(access())),
    );
    let processes = Unimock::new((
        ProcessTransportMock::list
            .each_call(matching!(_))
            .answers_arc(Arc::new(move |_, _| Ok(vec![process(terminal_id)]))),
        ProcessTransportMock::read_regular_file
            .each_call(matching!(_, _))
            .answers_arc({
                let observed = observed.clone();
                Arc::new(move |_, _, request: ProcessRegularFileRequest| {
                    observed
                        .lock()
                        .expect("deadline observations")
                        .push(request.timeout);
                    Ok(ProcessFileChunk {
                        bytes: b"output".to_vec(),
                        total_size: 6,
                    })
                })
            }),
    ));
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(processes));

    for wait in [Duration::ZERO, Duration::from_secs(30)] {
        let output = backend
            .read_terminal(request(terminal_id, wait))
            .await
            .expect("bounded provider read");
        assert_eq!(output.state, TerminalState::Ready);
    }

    let observed = observed.lock().expect("deadline observations");
    assert_eq!(observed.len(), 2);
    for (timeout, wait) in observed
        .iter()
        .copied()
        .zip([Duration::ZERO, Duration::from_secs(30)])
    {
        assert!(timeout > wait + Duration::from_secs(4));
        assert!(timeout <= wait + Duration::from_secs(5));
    }
}

#[tokio::test]
async fn terminal_reads_reject_excessive_wait_before_provider_access() {
    let backend = E2bSandboxBackend::with_transports(
        config(),
        Arc::new(Unimock::new(())),
        Arc::new(Unimock::new(())),
    );

    let error = backend
        .read_terminal(request(TerminalId::new(), Duration::from_secs(31)))
        .await
        .expect_err("excessive terminal wait must be rejected");

    assert!(matches!(
        error,
        Error::InvalidSeconds {
            field: "wait",
            minimum: 0,
            maximum: 30,
        }
    ));
}

fn request(terminal_id: TerminalId, wait: Duration) -> BackendOutputRequest {
    BackendOutputRequest {
        sandbox_provider_ref: ProviderRef::new("sandbox"),
        terminal_provider_ref: TerminalIdentity::new(41, terminal_id).provider_ref(),
        provider_log_path: format!("/var/lib/sandbox-e2b/terminals/{terminal_id}.log"),
        offset: 0,
        max_bytes: 1024,
        provider_log_limit: 1024,
        wait,
    }
}

fn process(terminal_id: TerminalId) -> ProcessInfo {
    ProcessInfo {
        pid: 41,
        tag: Some(format!("sandbox-terminal-{terminal_id}")),
    }
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
