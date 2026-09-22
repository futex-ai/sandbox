//! Sandbox creation validation regressions.

use std::{collections::HashMap, sync::Arc, time::Duration};

use sandbox_interface::{
    BackendCreateSandboxRequest, Error, OperationId, ResourceOwner, SANDBOX_ONE_SHOT_MAX_LIFETIME,
    SandboxBackend, SandboxConsumer, SandboxId, SandboxLifetime, SandboxNetworkPolicy,
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
        .create_sandbox(request(
            "missing",
            SandboxConsumer::Runtime,
            SandboxLifetime::IdleAutoPause,
        ))
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
        .recover_sandbox_create(request(
            "missing",
            SandboxConsumer::Runtime,
            SandboxLifetime::IdleAutoPause,
        ))
        .await;

    assert!(matches!(result, Err(Error::UnknownProfile)));
}

#[tokio::test]
async fn invalid_one_shot_bounds_are_rejected_before_create_provider_access() {
    let backend = E2bSandboxBackend::with_transports(
        config(),
        Arc::new(Unimock::new(())),
        Arc::new(Unimock::new(())),
    );

    for max_lifetime in [
        Duration::ZERO,
        SANDBOX_ONE_SHOT_MAX_LIFETIME + Duration::from_secs(1),
    ] {
        let result = backend
            .create_sandbox(request(
                "general",
                SandboxConsumer::Runtime,
                SandboxLifetime::OneShot { max_lifetime },
            ))
            .await;

        assert!(matches!(
            result,
            Err(Error::InvalidSeconds {
                field: "max_lifetime",
                minimum: 1,
                maximum: 3600,
            })
        ));
    }
}

#[tokio::test]
async fn recovery_revalidates_one_shot_lifetime_before_provider_access() {
    let backend = E2bSandboxBackend::with_transports(
        config(),
        Arc::new(Unimock::new(())),
        Arc::new(Unimock::new(())),
    );

    let result = backend
        .recover_sandbox_create(request(
            "general",
            SandboxConsumer::Runtime,
            SandboxLifetime::OneShot {
                max_lifetime: SANDBOX_ONE_SHOT_MAX_LIFETIME + Duration::from_secs(1),
            },
        ))
        .await;

    assert!(matches!(result, Err(Error::InvalidSeconds { .. })));
}

#[tokio::test]
async fn idle_lifetime_and_consumer_dispatch_without_an_inventory_preflight() {
    let control = Unimock::new(
        E2bControlApiMock::create_sandbox
            .next_call(matching!(_))
            .answers(&|_, request| {
                assert_eq!(
                    request.metadata.get("sandbox_consumer").map(String::as_str),
                    Some("browser")
                );
                assert_eq!(
                    request.metadata.get("sandbox_lifetime").map(String::as_str),
                    Some("idle_auto_pause")
                );
                assert!(
                    !request
                        .metadata
                        .contains_key("sandbox_one_shot_max_lifetime_seconds")
                );
                Ok(ControlSandboxAccess {
                    sandbox_id: "existing".to_owned(),
                    domain: "e2b.app".to_owned(),
                    envd_access_token: "call-local-token".to_owned(),
                    traffic_access_token: Some("traffic-token".to_owned()),
                })
            }),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(Unimock::new(())));

    backend
        .create_sandbox(request(
            "general",
            SandboxConsumer::Browser,
            SandboxLifetime::IdleAutoPause,
        ))
        .await
        .expect("browser sandbox metadata should be recoverable");
}

#[tokio::test]
async fn one_shot_duration_is_included_in_correlation_metadata() {
    let control = Unimock::new(
        E2bControlApiMock::create_sandbox
            .next_call(matching!(_))
            .answers(&|_, request| {
                assert_eq!(
                    request.metadata.get("sandbox_lifetime").map(String::as_str),
                    Some("one_shot")
                );
                assert_eq!(
                    request
                        .metadata
                        .get("sandbox_one_shot_max_lifetime_seconds")
                        .map(String::as_str),
                    Some("90")
                );
                Ok(ControlSandboxAccess {
                    sandbox_id: "existing".to_owned(),
                    domain: "e2b.app".to_owned(),
                    envd_access_token: "call-local-token".to_owned(),
                    traffic_access_token: Some("traffic-token".to_owned()),
                })
            }),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(Unimock::new(())));

    backend
        .create_sandbox(request(
            "general",
            SandboxConsumer::Runtime,
            SandboxLifetime::OneShot {
                max_lifetime: Duration::from_secs(90),
            },
        ))
        .await
        .expect("one-shot metadata should be recoverable");
}

fn request(
    profile: &str,
    consumer: SandboxConsumer,
    lifetime: SandboxLifetime,
) -> BackendCreateSandboxRequest {
    BackendCreateSandboxRequest {
        sandbox_id: SandboxId::new(),
        operation_id: OperationId::new(),
        owner: ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7()),
        consumer,
        lifetime,
        deployment_id: "deployment".to_owned(),
        profile: profile.to_owned(),
        network: SandboxNetworkPolicy::Open,
        snapshot_provider_ref: None,
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
