//! E2B sandbox network-policy mapping and recovery regressions.

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr},
    sync::{Arc, Mutex},
};

use sandbox_interface::{
    BackendCreateSandboxRequest, EgressDestination, Error, OperationId, ResourceOwner,
    SandboxBackend, SandboxConsumer, SandboxId, SandboxNetworkPolicy,
};
use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use crate::{
    ControlSandbox, ControlSandboxAccess, ControlSandboxState, E2bAdapterConfig, E2bControlApiMock,
    E2bProfile, SandboxMetadata,
};

use super::configured::E2bSandboxBackend;

#[tokio::test]
async fn open_create_preserves_profile_egress_and_omits_policy_metadata() {
    let control = Unimock::new((
        E2bControlApiMock::list_sandboxes
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
        E2bControlApiMock::create_sandbox
            .next_call(matching!(_))
            .answers(&|_, request| {
                assert!(request.allow_public_egress);
                assert_eq!(request.allowed_destinations, None);
                assert!(!request.metadata.contains_key("sandbox_network_policy"));
                Ok(access())
            }),
    ));
    let backend = backend(config(vec!["203.0.113.10/32"]), control);

    backend
        .create_sandbox(request(SandboxNetworkPolicy::Open))
        .await
        .expect("open policy should preserve profile egress");
}

#[tokio::test]
async fn allowlist_create_forwards_canonical_destinations_and_closed_egress() {
    let control = Unimock::new((
        E2bControlApiMock::list_sandboxes
            .next_call(matching!(_))
            .returns(Ok(Vec::new())),
        E2bControlApiMock::create_sandbox
            .next_call(matching!(_))
            .answers(&|_, request| {
                assert!(!request.allow_public_egress);
                assert_eq!(
                    request.allowed_destinations,
                    Some(vec!["192.0.2.10".to_owned(), "198.51.100.0/24".to_owned(),])
                );
                assert_eq!(request.denied_destinations, ["203.0.113.10/32".to_owned()]);
                assert!(
                    request
                        .metadata
                        .get("sandbox_network_policy")
                        .is_some_and(|value| value.starts_with("allowlist-v1:"))
                );
                Ok(access())
            }),
    ));
    let backend = backend(config(vec!["203.0.113.10/32"]), control);
    let policy = SandboxNetworkPolicy::allowlist(vec![
        EgressDestination::Ip("::ffff:192.0.2.10".parse().expect("mapped public IP")),
        EgressDestination::Cidr {
            address: IpAddr::V4(Ipv4Addr::new(198, 51, 100, 42)),
            prefix: 24,
        },
    ])
    .expect("valid policy");

    backend
        .create_sandbox(request(policy))
        .await
        .expect("allowlist should reach the control boundary");
}

#[tokio::test]
async fn domain_allowlist_create_is_rejected_before_provider_dispatch() {
    let backend = backend(config(vec!["203.0.113.10/32"]), Unimock::new(()));
    let policy = domain_policy("allowed.example.com");

    let error = backend
        .create_sandbox(request(policy.clone()))
        .await
        .expect_err("E2B must reject domain destinations");

    assert!(matches!(
        error,
        Error::UnsupportedNetworkPolicy { policy: rejected } if rejected == policy
    ));
}

#[tokio::test]
async fn domain_allowlist_recovery_is_rejected_before_provider_dispatch() {
    let backend = backend(config(vec!["203.0.113.10/32"]), Unimock::new(()));
    let policy = domain_policy("allowed.example.com");

    let error = backend
        .recover_sandbox_create(request(policy.clone()))
        .await
        .expect_err("E2B recovery must reject domain destinations");

    assert!(matches!(
        error,
        Error::UnsupportedNetworkPolicy { policy: rejected } if rejected == policy
    ));
}

#[tokio::test]
async fn recovery_revalidates_raw_policy_before_provider_dispatch() {
    let backend = backend(config(vec!["203.0.113.10/32"]), Unimock::new(()));
    let policy = SandboxNetworkPolicy::Allowlist {
        destinations: vec![EgressDestination::Domain("HTTPS://example.com".to_owned())],
    };

    assert!(matches!(
        backend.recover_sandbox_create(request(policy)).await,
        Err(Error::InvalidEgressDestination)
    ));
}

#[tokio::test]
async fn recovery_rejects_a_policy_that_differs_from_the_created_policy() {
    let created_metadata = Arc::new(Mutex::new(None::<SandboxMetadata>));
    let control = Unimock::new((
        E2bControlApiMock::list_sandboxes
            .each_call(matching!(_))
            .answers_arc({
                let created_metadata = created_metadata.clone();
                Arc::new(move |_, query| {
                    assert!(!query.contains_key("sandbox_network_policy"));
                    Ok(created_metadata
                        .lock()
                        .expect("created metadata lock")
                        .clone()
                        .map(|metadata| {
                            vec![ControlSandbox {
                                sandbox_id: "provider-sandbox".to_owned(),
                                state: ControlSandboxState::Running,
                                metadata,
                            }]
                        })
                        .unwrap_or_default())
                })
            }),
        E2bControlApiMock::create_sandbox
            .next_call(matching!(_))
            .answers_arc({
                let created_metadata = created_metadata.clone();
                Arc::new(move |_, request| {
                    *created_metadata.lock().expect("created metadata lock") =
                        Some(request.metadata);
                    Ok(access())
                })
            }),
    ));
    let backend = backend(config(vec!["203.0.113.10/32"]), control);
    let mut request = request(
        SandboxNetworkPolicy::allowlist(vec![
            EgressDestination::Ip(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10))),
            EgressDestination::Cidr {
                address: IpAddr::V4(Ipv4Addr::new(198, 51, 100, 42)),
                prefix: 24,
            },
        ])
        .expect("valid created policy"),
    );

    backend
        .create_sandbox(request.clone())
        .await
        .expect("initial create");
    request.network = SandboxNetworkPolicy::Allowlist {
        destinations: vec![
            EgressDestination::Ip("::ffff:192.0.2.10".parse().expect("mapped public IP")),
            EgressDestination::Cidr {
                address: IpAddr::V4(Ipv4Addr::new(198, 51, 100, 99)),
                prefix: 24,
            },
        ],
    };
    backend
        .recover_sandbox_create(request.clone())
        .await
        .expect("canonical equivalent recovery policy")
        .expect("created sandbox should recover");
    request.network = SandboxNetworkPolicy::allowlist(vec![EgressDestination::Ip(IpAddr::V4(
        Ipv4Addr::new(192, 0, 2, 11),
    ))])
    .expect("valid different policy");
    let error = backend
        .recover_sandbox_create(request)
        .await
        .expect_err("changed recovery policy must fail");

    assert!(matches!(error, Error::SandboxNetworkPolicyMismatch));
}

pub(super) fn backend(config: E2bAdapterConfig, control: Unimock) -> E2bSandboxBackend {
    E2bSandboxBackend::with_transports(config, Arc::new(control), Arc::new(Unimock::new(())))
}

pub(super) fn config(denied_destinations: Vec<&str>) -> E2bAdapterConfig {
    E2bAdapterConfig::new(
        "configured-e2b",
        "https://api.e2b.app",
        "api-key",
        HashMap::from([(
            "general".to_owned(),
            E2bProfile {
                template: "base".to_owned(),
                allow_public_egress: true,
                denied_destinations: denied_destinations.into_iter().map(str::to_owned).collect(),
            },
        )]),
        600,
    )
    .expect("valid adapter config")
}

pub(super) fn request(network: SandboxNetworkPolicy) -> BackendCreateSandboxRequest {
    BackendCreateSandboxRequest {
        sandbox_id: SandboxId::new(),
        operation_id: OperationId::new(),
        owner: ResourceOwner::agent(Uuid::now_v7(), Uuid::now_v7()),
        consumer: SandboxConsumer::Runtime,
        deployment_id: "deployment".to_owned(),
        profile: "general".to_owned(),
        network,
        snapshot_provider_ref: None,
    }
}

fn domain_policy(domain: &str) -> SandboxNetworkPolicy {
    SandboxNetworkPolicy::allowlist(vec![
        EgressDestination::Ip(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10))),
        EgressDestination::domain(domain).expect("valid test domain"),
    ])
    .expect("valid policy")
}

fn access() -> ControlSandboxAccess {
    ControlSandboxAccess {
        sandbox_id: "provider-sandbox".to_owned(),
        domain: "e2b.app".to_owned(),
        envd_access_token: "envd-token".to_owned(),
        traffic_access_token: "traffic-token".to_owned(),
    }
}
