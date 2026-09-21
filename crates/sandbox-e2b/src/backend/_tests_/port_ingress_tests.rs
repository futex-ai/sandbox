//! E2B authenticated port-ingress mapping coverage.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{BackendPortIngressRequest, Error, ProviderRef, SandboxBackend};
use unimock::{MockFn, Unimock, matching};

use crate::{ControlSandboxAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn ingress_uses_the_exact_port_and_distinct_traffic_token() {
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("provider"))
            .returns(Ok(access("provider"))),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(Unimock::new(())));

    let ingress = backend
        .port_ingress(BackendPortIngressRequest {
            sandbox_provider_ref: ProviderRef::new("provider"),
            port: 4173,
        })
        .await
        .expect("private port ingress");

    assert_eq!(ingress.upstream_url(), "https://4173-provider.e2b.app/");
    let credential = ingress.credential().expect("traffic credential");
    assert_eq!(credential.header_name(), "e2b-traffic-access-token");
    assert_eq!(credential.header_value(), "traffic-token");
}

#[tokio::test]
async fn ingress_rejects_a_mismatched_provider_identity() {
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("expected"))
            .returns(Ok(access("different"))),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(Unimock::new(())));

    let result = backend
        .port_ingress(BackendPortIngressRequest {
            sandbox_provider_ref: ProviderRef::new("expected"),
            port: 4173,
        })
        .await;

    assert!(matches!(result, Err(Error::Internal(_))));
}

#[tokio::test]
async fn ingress_rejects_a_provider_identity_that_changes_url_structure() {
    let control = Unimock::new(
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("provider/path"))
            .returns(Ok(access("provider/path"))),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(Unimock::new(())));

    let result = backend
        .port_ingress(BackendPortIngressRequest {
            sandbox_provider_ref: ProviderRef::new("provider/path"),
            port: 4173,
        })
        .await;

    assert!(matches!(result, Err(Error::Internal(_))));
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
        envd_access_token: "envd-token".to_owned(),
        traffic_access_token: "traffic-token".to_owned(),
    }
}
