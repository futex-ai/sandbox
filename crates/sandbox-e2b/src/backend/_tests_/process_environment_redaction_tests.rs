//! Environment-secret redaction at image-command diagnostic boundaries.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use sandbox_interface::{Error, ImageCommandFailure};
use unimock::{MockFn, Unimock, matching};

use crate::{
    E2bAdapterConfig, E2bProfile, ProcessCommand, ProcessConnection, ProcessOutputCapture,
    ProcessRunOutput, ProcessTransportMock,
};

use super::{
    configured::E2bSandboxBackend,
    image_command_diagnostic::{ImagePhase, run_process_phase},
};

#[tokio::test]
async fn image_failure_redacts_process_environment_values() {
    let processes = Arc::new(Unimock::new(
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(ProcessRunOutput {
                bytes: b"value=environment-secret".to_vec(),
                exit_code: Some(7),
                exited: true,
                output_truncated: false,
            })),
    ));
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(Unimock::new(())), processes);
    let command = ProcessCommand {
        command: "/bin/false".to_owned(),
        args: Vec::new(),
        cwd: None,
        envs: BTreeMap::from([("SAFE_SECRET".to_owned(), "environment-secret".to_owned())]),
        output_capture: ProcessOutputCapture::HardLimit { max_bytes: 4096 },
        timeout: Duration::from_secs(30),
        read_only: false,
    };

    let error = run_process_phase(
        &backend,
        ProcessConnection::new(
            "provider".to_owned(),
            "e2b.app".to_owned(),
            "access-token".to_owned(),
        ),
        command,
        ImagePhase::Setup,
    )
    .await
    .expect_err("failed command should return a safe diagnostic");

    assert!(matches!(
        error,
        Error::ImageSetupFailed {
            command: Some(ImageCommandFailure { ref output, .. }),
            ..
        } if output.as_deref() == Some("value=[REDACTED]")
    ));
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
