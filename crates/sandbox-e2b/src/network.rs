//! E2B network-policy validation, encoding, and recovery identity.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sandbox_interface::{EgressDestination, Error, Result, SandboxNetworkPolicy};
use sha2::{Digest, Sha256};

pub(crate) const PRIVATE_NETWORK_DENIES: &[&str] = &[
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
) -> Result<SandboxNetworkPolicy> {
    let policy = policy.validated()?;
    match &policy {
        SandboxNetworkPolicy::Open => Ok(policy),
        SandboxNetworkPolicy::Allowlist { destinations } => {
            validate_deny_overlap(destinations, deployment_denies)?;
            Ok(policy)
        }
        unsupported => Err(Error::UnsupportedNetworkPolicy {
            policy: unsupported.clone(),
        }),
    }
}

pub(crate) fn allow_out(policy: &SandboxNetworkPolicy) -> Option<Vec<String>> {
    match policy {
        SandboxNetworkPolicy::Open => None,
        SandboxNetworkPolicy::Allowlist { destinations } => {
            Some(destinations.iter().map(ToString::to_string).collect())
        }
        _ => None,
    }
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

fn validate_deny_overlap(
    destinations: &[EgressDestination],
    deployment_denies: &[String],
) -> Result<()> {
    let mut denies = PRIVATE_NETWORK_DENIES
        .iter()
        .map(|value| parse_range(value))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| Error::internal_message("invalid built-in E2B private deny range"))?;
    denies.extend(
        deployment_denies
            .iter()
            .map(|value| parse_range(value))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| Error::internal_message("invalid validated E2B profile deny range"))?,
    );
    for destination in destinations {
        let allowed = destination_range(destination);
        if allowed.is_some_and(|allowed| denies.iter().any(|denied| denied.overlaps(allowed))) {
            return Err(Error::EgressDestinationDenied);
        }
    }
    if destinations
        .iter()
        .any(|destination| matches!(destination, EgressDestination::Domain(_)))
        && denies
            .iter()
            .any(|denied| denied.contains(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))))
    {
        return Err(Error::EgressDestinationDenied);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct IpRange {
    address: IpAddr,
    prefix: u8,
}

impl IpRange {
    fn contains(self, address: IpAddr) -> bool {
        let prefix = match address {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        Self::new(address, prefix).is_some_and(|other| self.overlaps(other))
    }

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
