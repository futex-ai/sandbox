//! Typed per-session sandbox network policy seam.

use serde::{Deserialize, Serialize};

/// Typed per-session network policy applied at sandbox creation.
///
/// `Open` applies no additional per-session restriction and never bypasses
/// the deployment-owned egress policy: control-plane deny lists,
/// private-network denial, and profile egress rules apply unchanged. Future
/// allowlist values are reserved additive variants on this seam; services and
/// adapters must reject any unsupported variant with
/// [`Error::UnsupportedNetworkPolicy`](crate::Error::UnsupportedNetworkPolicy)
/// before any provider dispatch.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxNetworkPolicy {
    /// No additional per-session network restriction.
    #[default]
    Open,
}
