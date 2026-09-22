//! E2B network-address deny-overlap regressions.

use std::net::{IpAddr, Ipv4Addr};

use sandbox_interface::{EgressDestination, Error, SandboxBackend, SandboxNetworkPolicy};
use unimock::Unimock;

use super::sandbox_network_policy_tests::{backend, config, request};

#[tokio::test]
async fn private_and_deployment_deny_overlaps_fail_before_provider_dispatch() {
    let backend = backend(config(vec!["203.0.113.10/32"]), Unimock::new(()));
    let private_v4 = SandboxNetworkPolicy::allowlist(vec![EgressDestination::Ip(IpAddr::V4(
        Ipv4Addr::new(127, 0, 0, 1),
    ))])
    .expect("typed private IP");
    let private_v6 = SandboxNetworkPolicy::allowlist(vec![EgressDestination::Ip(
        "fc00::1".parse().expect("private IPv6 address"),
    )])
    .expect("typed private IPv6");
    let deployment = SandboxNetworkPolicy::allowlist(vec![EgressDestination::Cidr {
        address: IpAddr::V4(Ipv4Addr::new(203, 0, 113, 128)),
        prefix: 24,
    }])
    .expect("typed deployment range");

    for policy in [private_v4, private_v6, deployment] {
        assert!(matches!(
            backend.create_sandbox(request(policy)).await,
            Err(Error::EgressDestinationDenied)
        ));
    }
}

#[tokio::test]
async fn mapped_ipv6_allow_entries_overlap_ipv4_denies() {
    let private_backend = backend(config(vec!["203.0.113.10/32"]), Unimock::new(()));
    let mapped_deny_backend = backend(config(vec!["::ffff:203.0.113.10/128"]), Unimock::new(()));
    let mapped_ip = SandboxNetworkPolicy::Allowlist {
        destinations: vec![EgressDestination::Ip(
            "::ffff:127.0.0.1".parse().expect("mapped loopback"),
        )],
    };
    let mapped_range = SandboxNetworkPolicy::Allowlist {
        destinations: vec![EgressDestination::Cidr {
            address: "::fffe:0:0".parse().expect("mapped-spanning network"),
            prefix: 95,
        }],
    };

    for policy in [mapped_ip, mapped_range] {
        assert!(matches!(
            private_backend.create_sandbox(request(policy)).await,
            Err(Error::EgressDestinationDenied)
        ));
    }
    let denied_v4 = SandboxNetworkPolicy::allowlist(vec![EgressDestination::Ip(IpAddr::V4(
        Ipv4Addr::new(203, 0, 113, 10),
    ))])
    .expect("typed denied IPv4 address");
    assert!(matches!(
        mapped_deny_backend.create_sandbox(request(denied_v4)).await,
        Err(Error::EgressDestinationDenied)
    ));
}

#[tokio::test]
async fn domain_policy_rejects_a_denied_implicit_dns_resolver() {
    let backend = backend(config(vec!["8.8.8.0/24"]), Unimock::new(()));
    let policy = SandboxNetworkPolicy::allowlist(vec![
        EgressDestination::domain("example.com").expect("valid domain"),
    ])
    .expect("valid policy");

    assert!(matches!(
        backend.create_sandbox(request(policy)).await,
        Err(Error::EgressDestinationDenied)
    ));
}
