//! Image realization recovery-before-replay regressions.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{
    BackendRealizeImageRequest, Error, OperationId, ResourceOwner, SandboxBackend, SandboxId,
    SnapshotId,
};
use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use crate::{
    ControlSandbox, ControlSandboxAccess, ControlSandboxState, ControlSnapshot, E2bAdapterConfig,
    E2bControlApiMock, E2bProfile, ProcessRunOutput, ProcessTransportMock,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn completed_snapshot_is_recovered_before_build_side_effects() {
    let control = Unimock::new((
        E2bControlApiMock::list_sandboxes
            .next_call(matching!(_))
            .returns(Ok(vec![ControlSandbox {
                sandbox_id: "retained-source".to_owned(),
                state: ControlSandboxState::Running,
                metadata: Default::default(),
            }])),
        E2bControlApiMock::list_snapshots
            .next_call(matching!("retained-source", "sandbox-recovery"))
            .returns(Ok(vec![ControlSnapshot {
                snapshot_id: "completed-image".to_owned(),
            }])),
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("retained-source"))
            .returns(Ok(access("retained-source"))),
    ));
    let processes = Unimock::new(
        ProcessTransportMock::run
            .next_call(matching!(_, _))
            .answers(&|_, _, command| {
                assert!(command.args.iter().any(|arg| arg.contains("du -sbx")));
                Ok(size_output(8192))
            }),
    );

    let image = backend(control, processes)
        .realize_image(request(SandboxId::new()))
        .await
        .expect("completed image should recover without rerunning its build");

    assert_eq!(image.image_provider_ref.as_str(), "completed-image");
    assert_eq!(image.source_sandbox_cleanup_ref.as_str(), "retained-source");
    assert_eq!(image.size_bytes, 8192);
}

#[tokio::test]
async fn unresolved_snapshot_identity_retains_its_source_for_reconciliation() {
    let sandbox_id = SandboxId::new();
    let control = Unimock::new((
        E2bControlApiMock::list_sandboxes
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
        E2bControlApiMock::create_sandbox
            .next_call(matching!(_))
            .returns(Ok(access("unresolved-source"))),
        E2bControlApiMock::list_snapshots
            .next_call(matching!("unresolved-source", "sandbox-recovery"))
            .returns(Ok(vec![
                ControlSnapshot {
                    snapshot_id: "candidate-one".to_owned(),
                },
                ControlSnapshot {
                    snapshot_id: "candidate-two".to_owned(),
                },
            ])),
    ));

    let error = backend(control, Unimock::new(()))
        .realize_image(request(sandbox_id))
        .await
        .expect_err("multiple image candidates require caller reconciliation");

    assert!(matches!(
        error,
        Error::SnapshotReconciliationRequired {
            retained_sandbox: Some(ref retained),
        } if retained.sandbox_id == sandbox_id
            && retained.provider_ref.as_str() == "unresolved-source"
    ));
}

fn request(sandbox_id: SandboxId) -> BackendRealizeImageRequest {
    BackendRealizeImageRequest {
        sandbox_id,
        snapshot_id: SnapshotId::new(),
        operation_id: OperationId::new(),
        owner: ResourceOwner::platform(Uuid::now_v7()),
        deployment_id: "deployment".to_owned(),
        profile: "general".to_owned(),
        parent_image_provider_ref: None,
        input_files: Vec::new(),
        setup_script: "must-not-run".to_owned(),
        verify_commands: vec!["must-not-run".to_owned()],
        correlation_name: "sandbox-recovery".to_owned(),
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

fn size_output(size: u64) -> ProcessRunOutput {
    ProcessRunOutput {
        bytes: format!("__SANDBOX_IMAGE_SIZE__={size}\n").into_bytes(),
        exit_code: Some(0),
        exited: true,
        output_truncated: false,
    }
}
