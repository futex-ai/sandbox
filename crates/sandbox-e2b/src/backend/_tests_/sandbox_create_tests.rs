//! Sandbox creation validation regressions.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{
    BackendCreateSandboxRequest, Error, OperationId, ResourceOwner, SandboxBackend,
    SandboxConsumer, SandboxId, SandboxNetworkPolicy,
};
use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use crate::{ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile};

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
            consumer: SandboxConsumer::Runtime,
            deployment_id: "deployment".to_owned(),
            profile: "missing".to_owned(),
            network: SandboxNetworkPolicy::Open,
            snapshot_provider_ref: None,
        })
        .await;

    assert!(matches!(result, Err(Error::UnknownProfile)));
}

#[tokio::test]
async fn unknown_profile_recovery_is_rejected_before_provider_dispatch() {
    let backend = E2bSandboxBackend::with_transports(
        config(),
        Arc::new(Unimock::new(())),
        Arc::new(Unimock::new(())),
    );

    let result = backend
        .recover_sandbox_create(BackendCreateSandboxRequest {
            sandbox_id: SandboxId::new(),
            operation_id: OperationId::new(),
            owner: ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7()),
            consumer: SandboxConsumer::Runtime,
            deployment_id: "deployment".to_owned(),
            profile: "missing".to_owned(),
            network: SandboxNetworkPolicy::Open,
            snapshot_provider_ref: None,
        })
        .await;

    assert!(matches!(result, Err(Error::UnknownProfile)));
}

#[tokio::test]
async fn sandbox_create_dispatches_without_an_inventory_preflight() {
    let control = Unimock::new(
        E2bControlApiMock::create_sandbox
            .next_call(matching!(_))
            .answers(&|_, request| {
                assert_eq!(
                    request.metadata.get("sandbox_consumer").map(String::as_str),
                    Some("browser")
                );
                Ok(ControlSandboxAccess {
                    sandbox_id: "existing".to_owned(),
                    domain: "e2b.app".to_owned(),
                    envd_access_token: "call-local-token".to_owned(),
                    traffic_access_token: "traffic-token".to_owned(),
                })
            }),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(Unimock::new(())));

    backend
        .create_sandbox(BackendCreateSandboxRequest {
            sandbox_id: SandboxId::new(),
            operation_id: OperationId::new(),
            owner: ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7()),
            consumer: SandboxConsumer::Browser,
            deployment_id: "deployment".to_owned(),
            profile: "general".to_owned(),
            network: SandboxNetworkPolicy::Open,
            snapshot_provider_ref: None,
        })
        .await
        .expect("browser sandbox metadata should be recoverable");
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
