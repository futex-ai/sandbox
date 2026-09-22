//! Network-policy validation and canonicalization tests.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::{EGRESS_DESTINATION_MAX_ITEMS, EgressDestination, Error, SandboxNetworkPolicy};

#[test]
fn allowlist_constructor_canonicalizes_and_deduplicates_destinations() {
    let policy = SandboxNetworkPolicy::allowlist(vec![
        EgressDestination::Domain("*.example.com".to_owned()),
        EgressDestination::Cidr {
            address: IpAddr::V4(Ipv4Addr::new(203, 0, 113, 42)),
            prefix: 24,
        },
        EgressDestination::Ip(IpAddr::V6(Ipv6Addr::LOCALHOST)),
        EgressDestination::Domain("*.example.com".to_owned()),
    ])
    .expect("valid allowlist");

    assert_eq!(
        policy,
        SandboxNetworkPolicy::Allowlist {
            destinations: vec![
                EgressDestination::Ip(IpAddr::V6(Ipv6Addr::LOCALHOST)),
                EgressDestination::Cidr {
                    address: IpAddr::V4(Ipv4Addr::new(203, 0, 113, 0)),
                    prefix: 24,
                },
                EgressDestination::Domain("*.example.com".to_owned()),
            ],
        }
    );
}

#[test]
fn allowlist_constructor_enforces_the_entry_limit_before_deduplication() {
    let destinations = (0..=EGRESS_DESTINATION_MAX_ITEMS)
        .map(|_| EgressDestination::Ip(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))))
        .collect();

    let error = SandboxNetworkPolicy::allowlist(destinations)
        .expect_err("65 entries must exceed the limit");

    assert!(matches!(
        error,
        Error::InvalidLength {
            field: "network.allowlist.destinations",
            minimum: 0,
            maximum: EGRESS_DESTINATION_MAX_ITEMS,
        }
    ));
}

#[test]
fn allowlist_constructor_accepts_exactly_the_entry_limit() {
    let destinations = (0..EGRESS_DESTINATION_MAX_ITEMS)
        .map(|index| {
            EgressDestination::domain(format!("host-{index}.example.com"))
                .expect("valid unique domain")
        })
        .collect();

    SandboxNetworkPolicy::allowlist(destinations).expect("64 entries should be accepted");
}

#[test]
fn domain_constructor_accepts_exact_and_single_leading_wildcard_names() {
    assert_eq!(
        EgressDestination::domain("api.example.com").expect("exact domain"),
        EgressDestination::Domain("api.example.com".to_owned())
    );
    assert_eq!(
        EgressDestination::domain("*.example.com").expect("wildcard domain"),
        EgressDestination::Domain("*.example.com".to_owned())
    );
}

#[test]
fn malformed_domains_are_rejected() {
    let invalid = [
        "",
        "EXAMPLE.com",
        "*",
        "*.*.example.com",
        "api.*.example.com",
        ".example.com",
        "example.com.",
        "example.com:443",
        "https://example.com",
        "example.com/path",
        "127.0.0.1",
        "bad_name.example.com",
        "-bad.example.com",
        "bad-.example.com",
        " example.com",
    ];

    for domain in invalid {
        assert!(
            matches!(
                EgressDestination::domain(domain),
                Err(Error::InvalidEgressDestination)
            ),
            "{domain:?} should be rejected"
        );
    }
    assert!(matches!(
        EgressDestination::domain(format!("{}.example.com", "a".repeat(64))),
        Err(Error::InvalidEgressDestination)
    ));
    assert!(matches!(
        EgressDestination::domain(format!("{}.com", "a".repeat(250))),
        Err(Error::TextTooLarge {
            field: "network.allowlist.domain",
            limit: 253,
        })
    ));
}

#[test]
fn domain_constructor_accepts_the_maximum_total_length() {
    let domain = format!(
        "{}.{}.{}.{}",
        "a".repeat(63),
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(61)
    );

    assert_eq!(domain.len(), 253);
    EgressDestination::domain(domain).expect("253-byte domain should be accepted");
}

#[test]
fn cidr_constructor_validates_both_families_and_canonicalizes_host_bits() {
    let address = "2001:db8::abcd".parse().expect("test IPv6 address");

    assert_eq!(
        EgressDestination::cidr(address, 64).expect("valid IPv6 CIDR"),
        EgressDestination::Cidr {
            address: "2001:db8::".parse().expect("canonical IPv6 network"),
            prefix: 64,
        }
    );
    assert!(matches!(
        EgressDestination::cidr(address, 129),
        Err(Error::InvalidEgressDestination)
    ));
}

#[test]
fn raw_policy_values_are_revalidated_and_canonicalized() {
    let raw = SandboxNetworkPolicy::Allowlist {
        destinations: vec![EgressDestination::Cidr {
            address: IpAddr::V4(Ipv4Addr::new(198, 51, 100, 99)),
            prefix: 33,
        }],
    };

    assert!(matches!(
        raw.validated(),
        Err(Error::InvalidEgressDestination)
    ));
}

#[test]
fn network_policy_serde_round_trip_preserves_typed_destinations() {
    let policy = SandboxNetworkPolicy::allowlist(vec![
        EgressDestination::Ip(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10))),
        EgressDestination::domain("api.example.com").expect("valid domain"),
    ])
    .expect("valid allowlist");

    let encoded = serde_json::to_vec(&policy).expect("serialize policy");
    let decoded = serde_json::from_slice(&encoded).expect("deserialize policy");

    assert_eq!(policy, decoded);
}
