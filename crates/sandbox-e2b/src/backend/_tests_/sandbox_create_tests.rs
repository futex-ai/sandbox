//! Sandbox creation validation regressions.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{
    BackendCreateSandboxRequest, Error, OperationId, ResourceOwner, SandboxBackend, SandboxId,
    SandboxNetworkPolicy,
};
use unimock::Unimock;
use uuid::Uuid;

use crate::{E2bAdapterConfig, E2bProfile};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn unknown_profile_is_rejected_before_provider_dispatch() {
    let backend = E2bSandboxBackend::with_transports(
        config(),
        Arc::new(Unimock::new(())),
        Arc::new(Unimock::new(())),
    );

    let result = backend
        .create_sandbox(BackendCreateSandboxRequest {
            sandbox_id: SandboxId::new(),
            operation_id: OperationId::new(),
            owner: ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7()),
            deployment_id: "deployment".to_owned(),
            profile: "missing".to_owned(),
            network: SandboxNetworkPolicy::Open,
            snapshot_provider_ref: None,
        })
        .await;

    assert!(matches!(result, Err(Error::UnknownProfile)));
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
