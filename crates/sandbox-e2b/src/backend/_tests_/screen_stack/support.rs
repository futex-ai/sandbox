//! Shared screen-stack adapter test fixtures.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{BackendEnsureScreenStackRequest, ProviderRef};
use unimock::{MockFn, Unimock, matching};

use crate::backend::configured::E2bSandboxBackend;
use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, ProcessSplitOutput,
    ProcessTransportMock,
};

pub(super) const CAPABILITIES: &[u8] = br#"{"version":1,"features":{"dynamic_resize":true},"viewport":{"min_width":320,"max_width":3840,"min_height":240,"max_height":2160,"max_pixels":8294400}}"#;

pub(super) fn backend(output: ProcessSplitOutput) -> E2bSandboxBackend {
    let processes = Unimock::new((
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .returns(Ok(output)),
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .returns(Ok(split_failure(b"unsupported"))),
    ));
    E2bSandboxBackend::with_transports(config(), Arc::new(control()), Arc::new(processes))
}

pub(super) fn backend_with_capabilities(capabilities: ProcessSplitOutput) -> E2bSandboxBackend {
    let processes = Unimock::new((
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .returns(Ok(split_success(b"ready", b""))),
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .returns(Ok(capabilities)),
    ));
    E2bSandboxBackend::with_transports(config(), Arc::new(control()), Arc::new(processes))
}

pub(super) fn split_success(stdout: &[u8], stderr: &[u8]) -> ProcessSplitOutput {
    ProcessSplitOutput {
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
        exit_code: Some(0),
        exited: true,
        stdout_overflowed: false,
        stderr_overflowed: false,
    }
}

pub(super) fn split_failure(stderr: &[u8]) -> ProcessSplitOutput {
    ProcessSplitOutput {
        stderr: stderr.to_vec(),
        exit_code: Some(2),
        exited: true,
        ..ProcessSplitOutput::default()
    }
}

pub(super) fn control() -> Unimock {
    Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("provider"))
            .returns(Ok(ControlSandboxAccess {
                sandbox_id: "provider".to_owned(),
                domain: "e2b.app".to_owned(),
                envd_access_token: "envd-token".to_owned(),
                traffic_access_token: "traffic-token".to_owned(),
            })),
    )
}

pub(super) fn config() -> E2bAdapterConfig {
    E2bAdapterConfig::new(
        "configured-e2b",
        "https://api.e2b.app",
        "api-key",
        HashMap::from([(
            "general".to_owned(),
            E2bProfile {
                template: "screen-template".to_owned(),
                allow_public_egress: false,
                denied_destinations: vec!["203.0.113.10/32".to_owned()],
            },
        )]),
        600,
    )
    .expect("valid adapter config")
}

pub(super) fn request() -> BackendEnsureScreenStackRequest {
    BackendEnsureScreenStackRequest {
        sandbox_provider_ref: ProviderRef::new("provider"),
    }
}
