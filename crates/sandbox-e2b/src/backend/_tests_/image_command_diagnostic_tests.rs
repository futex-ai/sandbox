//! E2B image-command diagnostic and retention tests.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{
    BackendPrepareImageRequest, Error, ImageCommandFailure, ProviderRef, ResourceOwner,
    SandboxBackend, SandboxId,
};
use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, ProcessRunOutput,
    ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn setup_failure_retains_provider_sandbox_and_command_detail() {
    let sandbox_id = SandboxId::new();
    let control = retained_control("retained-source");
    let processes = Unimock::new(
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(failed_output(b"setup failed", Some(7)))),
    );

    let error = backend(control, processes)
        .prepare_image(request(
            sandbox_id,
            "retained-source",
            "exit 7",
            vec!["true"],
        ))
        .await
        .expect_err("setup failure should retain sandbox");

    assert!(matches!(
        error,
        Error::ImageSetupFailed {
            command: Some(ImageCommandFailure {
                exit_code: Some(7),
                ref output,
                output_truncated: false,
            }),
            retained_sandbox: Some(ref retained)
        } if retained.sandbox_id == sandbox_id
            && retained.provider_ref.as_str() == "retained-source"
            && output.as_deref() == Some("setup failed")
    ));
}

#[tokio::test]
async fn verification_failure_preserves_index_and_sanitized_missing_exit_detail() {
    let sandbox_id = SandboxId::new();
    let control = retained_control("retained-verify");
    let processes = Unimock::new((
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(success())),
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(success())),
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(failed_output(
                b"\x1b[31mbad\x1b[0m\r\nnext\x00\xff",
                None,
            ))),
    ));

    let error = backend(control, processes)
        .prepare_image(request(
            sandbox_id,
            "retained-verify",
            "true",
            vec!["true", "hang"],
        ))
        .await
        .expect_err("verification failure should retain safe command detail");

    assert!(matches!(
        error,
        Error::ImageVerificationFailed {
            index: 1,
            command: Some(ImageCommandFailure {
                exit_code: None,
                ref output,
                output_truncated: false,
            }),
            retained_sandbox: Some(ref retained),
        } if retained.sandbox_id == sandbox_id
            && retained.provider_ref.as_str() == "retained-verify"
            && output.as_deref() == Some("bad\nnext�")
    ));
}

#[tokio::test]
async fn setup_failure_preserves_transport_truncation() {
    let sandbox_id = SandboxId::new();
    let control = retained_control("retained-tail");
    let processes = Unimock::new(
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(ProcessRunOutput {
                bytes: b"final output".to_vec(),
                exit_code: Some(2),
                exited: true,
                output_truncated: true,
            })),
    );

    let error = backend(control, processes)
        .prepare_image(request(sandbox_id, "retained-tail", "exit 2", Vec::new()))
        .await
        .expect_err("truncated setup output should remain marked");

    assert!(matches!(
        error,
        Error::ImageSetupFailed {
            command: Some(ImageCommandFailure {
                output_truncated: true,
                ..
            }),
            ..
        }
    ));
}

#[tokio::test]
async fn setup_failure_redacts_connection_identity_and_access_token() {
    let sandbox_id = SandboxId::new();
    let control = retained_control("opaque-provider-id");
    let processes = Unimock::new(
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(failed_output(
                b"sandbox=opaque-provider-id token=call-local-token",
                Some(7),
            ))),
    );

    let error = backend(control, processes)
        .prepare_image(request(
            sandbox_id,
            "opaque-provider-id",
            "exit 7",
            Vec::new(),
        ))
        .await
        .expect_err("setup failure should return a safe diagnostic");

    assert!(matches!(
        error,
        Error::ImageSetupFailed {
            command: Some(ImageCommandFailure { ref output, .. }),
            ..
        } if output.as_deref()
            == Some("sandbox=[REDACTED] token=[REDACTED]")
    ));
}

#[tokio::test]
async fn truncated_setup_failure_redacts_a_token_suffix_at_the_tail_boundary() {
    let sandbox_id = SandboxId::new();
    let control = retained_control("opaque-provider-id");
    let processes = Unimock::new(
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .returns(Ok(ProcessRunOutput {
                bytes: b"local-token after-boundary".to_vec(),
                exit_code: Some(7),
                exited: true,
                output_truncated: true,
            })),
    );

    let error = backend(control, processes)
        .prepare_image(request(
            sandbox_id,
            "opaque-provider-id",
            "exit 7",
            Vec::new(),
        ))
        .await
        .expect_err("a boundary-spanning token should return a safe diagnostic");

    assert!(matches!(
        error,
        Error::ImageSetupFailed {
            command: Some(ImageCommandFailure { ref output, .. }),
            ..
        } if output.as_deref() == Some("[REDACTED] after-boundary")
    ));
}

fn retained_control(provider: &'static str) -> Unimock {
    Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!(_))
            .returns(Ok(access(provider))),
    )
}

fn request(
    sandbox_id: SandboxId,
    provider_ref: &str,
    setup_script: &str,
    verify_commands: Vec<&str>,
) -> BackendPrepareImageRequest {
    BackendPrepareImageRequest {
        sandbox_id,
        source_provider_ref: ProviderRef::new(provider_ref),
        owner: ResourceOwner::platform(Uuid::now_v7()),
        input_files: Vec::new(),
        setup_script: setup_script.to_owned(),
        verify_commands: verify_commands.into_iter().map(str::to_owned).collect(),
    }
}

fn failed_output(bytes: &[u8], exit_code: Option<i32>) -> ProcessRunOutput {
    ProcessRunOutput {
        bytes: bytes.to_vec(),
        exit_code,
        exited: exit_code.is_some(),
        output_truncated: false,
    }
}

fn success() -> ProcessRunOutput {
    ProcessRunOutput {
        bytes: Vec::new(),
        exit_code: Some(0),
        exited: true,
        output_truncated: false,
    }
}

fn backend(control: Unimock, processes: Unimock) -> E2bSandboxBackend {
    E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(processes))
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

fn access(sandbox_id: &str) -> ControlSandboxAccess {
    ControlSandboxAccess {
        sandbox_id: sandbox_id.to_owned(),
        domain: "e2b.app".to_owned(),
        envd_access_token: "call-local-token".to_owned(),
        traffic_access_token: "traffic-token".to_owned(),
    }
}
