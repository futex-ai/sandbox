//! E2B network-policy validation, encoding, and recovery identity.
//!
//! IP and CIDR allowlists are supported; domain destinations fail closed.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sandbox_interface::{
    EgressDestination, Error as InterfaceError, Result as InterfaceResult, SandboxNetworkPolicy,
};
use sha2::{Digest, Sha256};

use crate::error::{Error as AdapterError, Result as AdapterResult};

const PRIVATE_NETWORK_DENIES: &[&str] = &[
    "10.0.0.0/8",
    "100.64.0.0/10",
    "127.0.0.0/8",
    "169.254.0.0/16",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "224.0.0.0/4",
    "::1/128",
    "fc00::/7",
    "fe80::/10",
];

pub(crate) fn validate(
    policy: &SandboxNetworkPolicy,
    deployment_denies: &[String],
) -> InterfaceResult<SandboxNetworkPolicy> {
    let policy = policy.validated()?;
    match &policy {
        SandboxNetworkPolicy::Open => Ok(policy),
        SandboxNetworkPolicy::Allowlist { destinations } => {
            if destinations
                .iter()
                .any(|destination| matches!(destination, EgressDestination::Domain(_)))
            {
                return Err(InterfaceError::UnsupportedNetworkPolicy {
                    policy: policy.clone(),
                });
            }
            validate_deny_overlap(destinations, deployment_denies)?;
            Ok(policy)
        }
        unsupported => Err(InterfaceError::UnsupportedNetworkPolicy {
            policy: unsupported.clone(),
        }),
    }
}

/// Clones canonical destinations for the independently validated control seam.
pub(crate) fn destinations(policy: &SandboxNetworkPolicy) -> Option<Vec<EgressDestination>> {
    match policy {
        SandboxNetworkPolicy::Open => None,
        SandboxNetworkPolicy::Allowlist { destinations } => Some(destinations.clone()),
        _ => None,
    }
}

/// Canonical E2B create-body fields after direct-client safety validation.
pub(crate) struct ValidatedControlNetwork {
    /// Merged private and deployment deny rules.
    pub(crate) deny_out: Vec<String>,
    /// Canonical IP/CIDR allow rules, absent for an Open policy.
    pub(crate) allow_out: Option<Vec<String>>,
}

/// Revalidates public control-client network input before transport.
pub(crate) fn validate_control_create(
    allow_public_egress: bool,
    allowed_destinations: Option<Vec<EgressDestination>>,
    deployment_denies: Vec<String>,
) -> AdapterResult<ValidatedControlNetwork> {
    let mut canonical_denies = Vec::with_capacity(deployment_denies.len());
    for destination in deployment_denies {
        let Some(destination) = canonical_ip_destination(&destination) else {
            return Err(AdapterError::InvalidRequest);
        };
        canonical_denies.push(destination);
    }
    canonical_denies.sort();
    canonical_denies.dedup();

    let allow_out = match allowed_destinations {
        None => None,
        Some(destinations) => {
            if allow_public_egress {
                return Err(AdapterError::InvalidRequest);
            }
            let policy = match SandboxNetworkPolicy::allowlist(destinations) {
                Ok(policy) => policy,
                Err(_) => return Err(AdapterError::InvalidRequest),
            };
            let policy = match validate(&policy, &canonical_denies) {
                Ok(policy) => policy,
                Err(_) => return Err(AdapterError::InvalidRequest),
            };
            policy_destinations(&policy)
        }
    };

    let mut deny_out = PRIVATE_NETWORK_DENIES
        .iter()
        .map(|destination| (*destination).to_owned())
        .collect::<Vec<_>>();
    deny_out.extend(canonical_denies);
    deny_out.sort();
    deny_out.dedup();
    Ok(ValidatedControlNetwork {
        deny_out,
        allow_out,
    })
}

/// Canonicalizes one strict bare IP or CIDR rule.
pub(crate) fn canonical_ip_destination(destination: &str) -> Option<String> {
    let destination = destination.trim();
    let value = match destination.split_once('/') {
        Some((address, prefix)) => EgressDestination::Cidr {
            address: address.parse().ok()?,
            prefix: prefix.parse().ok()?,
        },
        None => EgressDestination::Ip(destination.parse().ok()?),
    };
    Some(value.validated().ok()?.to_string())
}

pub(crate) fn allow_public_egress(policy: &SandboxNetworkPolicy, configured: bool) -> bool {
    matches!(policy, SandboxNetworkPolicy::Open) && configured
}

pub(crate) fn fingerprint(policy: &SandboxNetworkPolicy) -> Option<String> {
    let SandboxNetworkPolicy::Allowlist { destinations } = policy else {
        return None;
    };
    let mut hasher = Sha256::new();
    hasher.update(b"sandbox-egress-allowlist-v1\0");
    for destination in destinations {
        match destination {
            EgressDestination::Ip(_) => hasher.update(b"ip\0"),
            EgressDestination::Cidr { .. } => hasher.update(b"cidr\0"),
            EgressDestination::Domain(_) => hasher.update(b"domain\0"),
        }
        let value = destination.to_string();
        hasher.update(value.as_bytes());
        hasher.update(b"\0");
    }
    Some(format!(
        "allowlist-v1:{}",
        URL_SAFE_NO_PAD.encode(hasher.finalize())
    ))
}

fn policy_destinations(policy: &SandboxNetworkPolicy) -> Option<Vec<String>> {
    match policy {
        SandboxNetworkPolicy::Allowlist { destinations } => {
            Some(destinations.iter().map(ToString::to_string).collect())
        }
        SandboxNetworkPolicy::Open => None,
        _ => None,
    }
}

fn validate_deny_overlap(
    destinations: &[EgressDestination],
    deployment_denies: &[String],
) -> InterfaceResult<()> {
    let mut denies = PRIVATE_NETWORK_DENIES
        .iter()
        .map(|value| parse_range(value))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            InterfaceError::internal_message("invalid built-in E2B private deny range")
        })?;
    denies.extend(
        deployment_denies
            .iter()
            .map(|value| parse_range(value))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                InterfaceError::internal_message("invalid validated E2B profile deny range")
            })?,
    );
    for destination in destinations {
        let allowed = destination_range(destination);
        if allowed.is_some_and(|allowed| denies.iter().any(|denied| denied.overlaps(allowed))) {
            return Err(InterfaceError::EgressDestinationDenied);
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct IpRange {
    address: IpAddr,
    prefix: u8,
}

impl IpRange {
    fn overlaps(self, other: Self) -> bool {
        match (self.address, other.address) {
            (IpAddr::V4(left), IpAddr::V4(right)) => {
                let prefix = self.prefix.min(other.prefix);
                masked_v4(left, prefix) == masked_v4(right, prefix)
            }
            (IpAddr::V6(left), IpAddr::V6(right)) => {
                let prefix = self.prefix.min(other.prefix);
                masked_v6(left, prefix) == masked_v6(right, prefix)
            }
            (IpAddr::V4(_), IpAddr::V6(_)) | (IpAddr::V6(_), IpAddr::V4(_)) => {
                self.as_mapped_v6().overlaps(other.as_mapped_v6())
            }
        }
    }

    fn new(address: IpAddr, prefix: u8) -> Option<Self> {
        match address {
            IpAddr::V4(_) if prefix > 32 => None,
            IpAddr::V6(_) if prefix > 128 => None,
            IpAddr::V6(address) if prefix >= 96 => match address.to_ipv4_mapped() {
                Some(address) => Some(Self {
                    address: IpAddr::V4(address),
                    prefix: prefix - 96,
                }),
                None => Some(Self {
                    address: IpAddr::V6(address),
                    prefix,
                }),
            },
            IpAddr::V4(_) | IpAddr::V6(_) => Some(Self { address, prefix }),
        }
    }

    fn as_mapped_v6(self) -> Self {
        match self.address {
            IpAddr::V4(address) => Self {
                address: IpAddr::V6(address.to_ipv6_mapped()),
                prefix: self.prefix + 96,
            },
            IpAddr::V6(_) => self,
        }
    }
}

fn destination_range(destination: &EgressDestination) -> Option<IpRange> {
    match destination {
        EgressDestination::Ip(address) => IpRange::new(
            *address,
            match address {
                IpAddr::V4(_) => 32,
                IpAddr::V6(_) => 128,
            },
        ),
        EgressDestination::Cidr { address, prefix } => IpRange::new(*address, *prefix),
        EgressDestination::Domain(_) => None,
    }
}

fn parse_range(value: &str) -> Option<IpRange> {
    let (address, prefix) = match value.split_once('/') {
        Some((address, prefix)) => (address.parse().ok()?, prefix.parse().ok()?),
        None => {
            let address = value.parse().ok()?;
            let prefix = match address {
                IpAddr::V4(_) => 32,
                IpAddr::V6(_) => 128,
            };
            (address, prefix)
        }
    };
    IpRange::new(address, prefix)
}

fn masked_v4(address: Ipv4Addr, prefix: u8) -> u32 {
    let mask = match prefix {
        0 => 0,
        _ => u32::MAX << (32 - prefix),
    };
    u32::from(address) & mask
}

fn masked_v6(address: Ipv6Addr, prefix: u8) -> u128 {
    let mask = match prefix {
        0 => 0,
        _ => u128::MAX << (128 - prefix),
    };
    u128::from(address) & mask
}
