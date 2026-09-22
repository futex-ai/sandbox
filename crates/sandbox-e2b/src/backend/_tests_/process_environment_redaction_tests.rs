//! Arbitrary process output never becomes handled image-error text.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use sandbox_interface::Error;
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
async fn image_failure_reports_only_metadata_for_arbitrary_output() {
    for (bytes, truncated) in [
        (b"environment-secret".as_slice(), false),
        (b"prefix-\x1b[31mpassword\x1b[0m123".as_slice(), false),
        (b"abcdef".as_slice(), false),
        (b"token%2Fa%2Bb".as_slice(), false),
        (b"password123".as_slice(), true),
        (b"\r\n\xffunknown-credential".as_slice(), true),
        (b"".as_slice(), false),
    ] {
        let processes = Arc::new(Unimock::new(
            ProcessTransportMock::run
                .next_call(matching!(_, _))
                .returns(Ok(ProcessRunOutput {
                    bytes: bytes.to_vec(),
                    exit_code: Some(7),
                    exited: true,
                    output_truncated: truncated,
                })),
        ));
        let backend =
            E2bSandboxBackend::with_transports(config(), Arc::new(Unimock::new(())), processes);
        let command = ProcessCommand {
            command: "/bin/false".to_owned(),
            args: Vec::new(),
            cwd: None,
            envs: BTreeMap::from([
                ("FIRST".to_owned(), "prefix-password123".to_owned()),
                ("SECOND".to_owned(), "password".to_owned()),
                ("THIRD".to_owned(), "environment-secret".to_owned()),
            ]),
            output_capture: ProcessOutputCapture::Tail { max_bytes: 4096 },
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
        .expect_err("failed command should return metadata");
        let Error::ImageSetupFailed {
            command: Some(failure),
            retained_sandbox: None,
        } = error
        else {
            panic!("expected setup failure metadata");
        };
        assert_eq!(
            serde_json::to_value(&failure).expect("failure metadata"),
            serde_json::json!({
                "exit_code": 7,
                "exited": true,
                "output_bytes": bytes.len(),
                "output_truncated": truncated,
            })
        );
        assert_eq!(
            format!("{failure:?}"),
            format!(
                "ImageCommandFailure {{ exit_code: Some(7), exited: true, output_bytes: {}, output_truncated: {} }}",
                bytes.len(),
                truncated,
            )
        );
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
