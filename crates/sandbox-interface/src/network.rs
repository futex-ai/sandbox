//! Typed per-session sandbox network policy seam.

use std::{
    fmt,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

use serde::{Deserialize, Serialize};
use url::Host;

use crate::{Error, Result};

/// Maximum number of caller-supplied destinations in one allowlist.
pub const EGRESS_DESTINATION_MAX_ITEMS: usize = 64;

/// Maximum byte length of one exact or wildcard DNS destination.
pub const EGRESS_DOMAIN_MAX_BYTES: usize = 253;

/// One canonical outbound destination selected by a sandbox allowlist.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EgressDestination {
    /// One exact IPv4 or IPv6 address; mapped IPv6 canonicalizes to IPv4.
    Ip(IpAddr),
    /// One IPv4 or IPv6 network, canonicalized to its network address.
    ///
    /// IPv4-mapped IPv6 CIDRs contained by the mapped prefix canonicalize to
    /// their equivalent IPv4 network.
    Cidr {
        /// Address whose network portion identifies the destination range.
        address: IpAddr,
        /// IPv4 `0..=32` or IPv6 `0..=128` prefix length.
        prefix: u8,
    },
    /// One lowercase DNS name with an optional single leading `*.` label.
    ///
    /// A wildcard matches subdomains at any depth but not the apex name.
    Domain(String),
}

impl EgressDestination {
    /// Builds a canonical CIDR destination after validating its prefix length.
    pub fn cidr(address: IpAddr, prefix: u8) -> Result<Self> {
        Self::Cidr { address, prefix }.validated()
    }

    /// Builds a validated lowercase exact or leading-wildcard DNS destination.
    ///
    /// Canonical and legacy URL-style IP literals are rejected.
    pub fn domain(domain: impl Into<String>) -> Result<Self> {
        Self::Domain(domain.into()).validated()
    }

    /// Revalidates and canonicalizes a destination from any construction path.
    pub fn validated(&self) -> Result<Self> {
        match self {
            Self::Ip(address) => Ok(Self::Ip(canonical_ip(*address))),
            Self::Cidr { address, prefix } => {
                let (address, prefix) = canonical_cidr(*address, *prefix)?;
                Ok(Self::Cidr { address, prefix })
            }
            Self::Domain(domain) => {
                validate_domain(domain)?;
                Ok(Self::Domain(domain.clone()))
            }
        }
    }
}

impl fmt::Display for EgressDestination {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ip(address) => write!(formatter, "{address}"),
            Self::Cidr { address, prefix } => write!(formatter, "{address}/{prefix}"),
            Self::Domain(domain) => formatter.write_str(domain),
        }
    }
}

/// Typed per-session network policy applied at sandbox creation.
///
/// `Open` applies no additional per-session restriction and never bypasses
/// deployment-owned private-network or profile egress rules. `Allowlist`
/// denies ordinary outbound traffic by default and permits only its named
/// destinations, subject to those same deployment rules.
///
/// Domain destinations filter HTTP on port 80 by the `Host` header and TLS on
/// port 443 by SNI. Other ports, UDP protocols such as QUIC, and traffic that
/// does not present either hostname are filtered only by IP or CIDR rules.
/// Destination support is adapter-specific. An adapter that cannot enforce a
/// policy or one of its destination kinds without weakening deployment rules
/// must reject the complete policy with
/// [`Error::UnsupportedNetworkPolicy`](crate::Error::UnsupportedNetworkPolicy)
/// before provider mutation. The same rule applies to future variants.
#[non_exhaustive]
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxNetworkPolicy {
    /// No additional per-session network restriction.
    #[default]
    Open,
    /// Deny outbound traffic by default and allow only canonical destinations.
    Allowlist {
        /// At most 64 exact IPs, canonical CIDRs, or validated DNS names.
        destinations: Vec<EgressDestination>,
    },
}

impl SandboxNetworkPolicy {
    /// Builds a bounded allowlist with canonical, sorted, unique destinations.
    pub fn allowlist(destinations: Vec<EgressDestination>) -> Result<Self> {
        Self::Allowlist { destinations }.validated()
    }

    /// Revalidates and canonicalizes a policy from any construction path.
    pub fn validated(&self) -> Result<Self> {
        match self {
            Self::Open => Ok(Self::Open),
            Self::Allowlist { destinations } => {
                if destinations.len() > EGRESS_DESTINATION_MAX_ITEMS {
                    return Err(Error::InvalidLength {
                        field: "network.allowlist.destinations",
                        minimum: 0,
                        maximum: EGRESS_DESTINATION_MAX_ITEMS,
                    });
                }
                let mut destinations = destinations
                    .iter()
                    .map(EgressDestination::validated)
                    .collect::<Result<Vec<_>>>()?;
                destinations.sort();
                destinations.dedup();
                Ok(Self::Allowlist { destinations })
            }
        }
    }
}

fn canonical_ip(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V4(address) => IpAddr::V4(address),
        IpAddr::V6(address) => address
            .to_ipv4_mapped()
            .map_or(IpAddr::V6(address), IpAddr::V4),
    }
}

fn canonical_cidr(address: IpAddr, prefix: u8) -> Result<(IpAddr, u8)> {
    match address {
        IpAddr::V4(address) if prefix <= 32 => {
            let mask = prefix_mask_v4(prefix);
            Ok((
                IpAddr::V4(Ipv4Addr::from(u32::from(address) & mask)),
                prefix,
            ))
        }
        IpAddr::V6(address) if prefix <= 128 => {
            if let Some(address) = address.to_ipv4_mapped()
                && prefix >= 96
            {
                return canonical_cidr(IpAddr::V4(address), prefix - 96);
            }
            let mask = prefix_mask_v6(prefix);
            Ok((
                IpAddr::V6(Ipv6Addr::from(u128::from(address) & mask)),
                prefix,
            ))
        }
        IpAddr::V4(_) | IpAddr::V6(_) => Err(Error::InvalidEgressDestination),
    }
}

fn prefix_mask_v4(prefix: u8) -> u32 {
    match prefix {
        0 => 0,
        _ => u32::MAX << (32 - prefix),
    }
}

fn prefix_mask_v6(prefix: u8) -> u128 {
    match prefix {
        0 => 0,
        _ => u128::MAX << (128 - prefix),
    }
}

fn validate_domain(domain: &str) -> Result<()> {
    if domain.len() > EGRESS_DOMAIN_MAX_BYTES {
        return Err(Error::TextTooLarge {
            field: "network.allowlist.domain",
            limit: EGRESS_DOMAIN_MAX_BYTES,
        });
    }
    let name = domain.strip_prefix("*.").unwrap_or(domain);
    if name.is_empty()
        || !name.is_ascii()
        || name.parse::<IpAddr>().is_ok()
        || matches!(Host::parse(name), Ok(Host::Ipv4(_) | Host::Ipv6(_)))
        || name.split('.').any(invalid_dns_label)
        || name.contains('*')
    {
        return Err(Error::InvalidEgressDestination);
    }
    Ok(())
}

fn invalid_dns_label(label: &str) -> bool {
    label.is_empty()
        || label.len() > 63
        || !label
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !label
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !label
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

#[cfg(test)]
#[path = "_tests_/network_tests.rs"]
mod network_tests;
