//! Typed E2B control API request and response DTOs.

use std::{collections::BTreeMap, fmt};

use sandbox_interface::EgressDestination;
use serde::{Deserialize, Serialize};

/// Opaque, non-secret consumer metadata attached to E2B sandboxes.
pub type SandboxMetadata = BTreeMap<String, String>;

/// Provider-specific input for creating one secure sandbox.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlCreateSandbox {
    /// Deployment-owned E2B template or snapshot identifier.
    pub template_id: String,
    /// Exact non-secret operation metadata.
    pub metadata: SandboxMetadata,
    /// Whether ordinary public internet egress is enabled.
    pub allow_public_egress: bool,
    /// Additional deployment-owned denied destinations.
    pub denied_destinations: Vec<String>,
    /// Typed per-session allow destinations, absent for open policy.
    ///
    /// The concrete client revalidates and canonicalizes these values before
    /// transport.
    pub allowed_destinations: Option<Vec<EgressDestination>>,
    /// Auto-pause timeout in seconds.
    pub idle_timeout_seconds: u32,
}

/// Call-local access material returned by sandbox create or connect.
#[derive(Clone, Eq, PartialEq)]
pub struct ControlSandboxAccess {
    /// Lowercase DNS-route-safe E2B sandbox identifier.
    pub sandbox_id: String,
    /// Control-reported domain retained for transport tests and diagnostics.
    /// Backend credential routing always uses validated adapter configuration.
    pub domain: String,
    /// Nonblank secret envd access token; never persist or log this value.
    pub envd_access_token: String,
    /// Nonblank secret traffic access token; never persist or log this value.
    pub traffic_access_token: String,
}

impl fmt::Debug for ControlSandboxAccess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ControlSandboxAccess")
            .field("sandbox_id", &"[redacted]")
            .field("domain", &self.domain)
            .field("envd_access_token", &"[redacted]")
            .field("traffic_access_token", &"[redacted]")
            .finish()
    }
}

/// E2B sandbox lifecycle state understood by the adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlSandboxState {
    /// The sandbox is running and accepts envd requests.
    Running,
    /// The sandbox is paused with persisted memory.
    Paused,
}

/// Provider sandbox identity and lifecycle state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlSandbox {
    /// Lowercase DNS-route-safe E2B sandbox identifier.
    pub sandbox_id: String,
    /// Current provider lifecycle state.
    pub state: ControlSandboxState,
    /// Opaque non-secret metadata retained for consumer reconciliation.
    pub metadata: SandboxMetadata,
}

/// Call-local envd access for a provider-verified running sandbox.
#[derive(Clone, Eq, PartialEq)]
pub struct ControlSandboxReadAccess {
    /// Lowercase DNS-route-safe E2B sandbox identifier.
    pub sandbox_id: String,
    /// Control-reported domain retained for transport tests and diagnostics.
    /// Backend credential routing always uses validated adapter configuration.
    pub domain: String,
    /// Secret envd access token; never persist or log this value.
    pub envd_access_token: String,
}

impl fmt::Debug for ControlSandboxReadAccess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ControlSandboxReadAccess")
            .field("sandbox_id", &"[redacted]")
            .field("domain", &self.domain)
            .field("envd_access_token", &"[redacted]")
            .finish()
    }
}

/// One immutable E2B snapshot version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlSnapshot {
    /// Route-safe opaque snapshot template identifier including its version tag.
    pub snapshot_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreateSandboxBody {
    #[serde(rename = "templateID")]
    pub(super) template_id: String,
    pub(super) metadata: SandboxMetadata,
    pub(super) secure: bool,
    #[serde(rename = "allow_internet_access")]
    pub(super) allow_internet_access: bool,
    pub(super) network: NetworkBody,
    pub(super) auto_pause: bool,
    pub(super) auto_pause_memory: bool,
    pub(super) auto_resume: AutoResumeBody,
    pub(super) timeout: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct NetworkBody {
    pub(super) allow_public_traffic: bool,
    pub(super) deny_out: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) allow_out: Option<Vec<String>>,
}

#[derive(Serialize)]
pub(super) struct AutoResumeBody {
    pub(super) enabled: bool,
}

#[derive(Serialize)]
pub(super) struct ConnectBody {
    pub(super) timeout: u32,
}

#[derive(Serialize)]
pub(super) struct PauseBody {
    pub(super) memory: bool,
}

#[derive(Serialize)]
pub(super) struct SnapshotBody<'a> {
    pub(super) name: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SandboxAccessBody {
    #[serde(rename = "sandboxID")]
    pub(super) sandbox_id: String,
    pub(super) envd_access_token: Option<String>,
    pub(super) traffic_access_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SandboxDetailBody {
    #[serde(rename = "sandboxID")]
    pub(super) sandbox_id: String,
    pub(super) state: SandboxStateBody,
    pub(super) envd_access_token: Option<String>,
    #[serde(default)]
    pub(super) metadata: SandboxMetadata,
    pub(super) lifecycle: Option<SandboxLifecycleBody>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SandboxLifecycleBody {
    pub(super) auto_resume: bool,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SandboxStateBody {
    Running,
    Paused,
}

#[derive(Deserialize)]
pub(super) struct ListedSandboxBody {
    #[serde(rename = "sandboxID")]
    pub(super) sandbox_id: String,
    pub(super) state: SandboxStateBody,
    #[serde(default)]
    pub(super) metadata: SandboxMetadata,
}

#[derive(Deserialize)]
pub(super) struct SnapshotInfoBody {
    #[serde(rename = "snapshotID")]
    pub(super) snapshot_id: String,
    #[serde(default)]
    pub(super) names: Vec<String>,
}
