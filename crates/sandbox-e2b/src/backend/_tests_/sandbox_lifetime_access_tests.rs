//! One-shot lifecycle-safe access mapping regressions.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{ProviderRef, SandboxBackend, SandboxState};
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, E2bSandboxBackend,
};

#[tokio::test]
async fn resume_accepts_non_mutating_access_without_a_traffic_token() {
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("one-shot"))
            .returns(Ok(ControlSandboxAccess {
                sandbox_id: "one-shot".to_owned(),
                domain: "e2b.app".to_owned(),
                envd_access_token: "envd-token".to_owned(),
                traffic_access_token: None,
            })),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(Unimock::new(())));

    let resumed = backend
        .resume_sandbox(ProviderRef::new("one-shot"))
        .await
        .expect("running one-shot read access should satisfy resume");

    assert_eq!(resumed.state, SandboxState::Ready);
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
