//! Typed Boat request, inventory, and resource data.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Stable labels preserved through Boat create, inspection, and inventory.
pub type BoatLabels = BTreeMap<String, String>;

/// Fully validated disposable create payload.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoatCreateSandbox {
    /// Published Boat machine type.
    #[serde(rename = "type")]
    pub machine_type: String,
    /// Positive bounded one-shot timeout.
    pub ttl_seconds: u32,
    /// Excludes account credentials and account environment data.
    pub no_env: bool,
    /// Disables automatic snapshot capture.
    pub snapshots: bool,
    /// Disables retained monitoring data.
    pub monitoring: bool,
    /// Exact durable request identity.
    pub labels: BoatLabels,
    /// Exact saved-snapshot name, when restoring.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Durable create operation, sent only in the idempotency header.
    #[serde(skip)]
    pub operation: String,
}

/// Provider sandbox fields used by supported capabilities.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoatSandbox {
    /// Validated public Boat sandbox identity.
    pub id: String,
    /// Typed provider lifecycle state.
    pub state: BoatSandboxState,
    /// Labels are absent on older or terminal provider records.
    #[serde(default)]
    pub labels: BoatLabels,
    /// Current machine size, when the provider supplies it.
    #[serde(rename = "type")]
    pub machine_type: Option<String>,
    /// Provider auto-stop timeout, when supplied.
    pub ttl_seconds: Option<u32>,
    /// Whether this source can be captured.
    pub snapshots: Option<bool>,
    /// Whether monitoring is enabled.
    pub monitoring: Option<bool>,
}

/// Boat sandbox states, including terminal cancelled creates.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BoatSandboxState {
    /// Request initialization.
    Init,
    /// Machine allocation.
    Provisioning,
    /// Allocated machine initialization.
    Provisioned,
    /// Source data restoration.
    Cloning,
    /// Workload can run.
    Ready,
    /// Workload has no current activity.
    Idle,
    /// Workload has current activity.
    Running,
    /// Provider stop is in progress.
    Archiving,
    /// Workload stopped and snapshots-disabled disk is lost.
    Archived,
    /// Provider runtime failure.
    Error,
    /// Terminal cancelled allocation.
    Cancelled,
}

/// Initial saved-snapshot mutation.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoatSaveSnapshot {
    /// Caller-owned capture-enabled source.
    pub sandbox_id: String,
    /// Stable unique correlation name owned by this operation.
    pub name: String,
}

/// Saved snapshot independent of the source's subsequent lifetime.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoatSavedSnapshot {
    /// Exact saved-snapshot name used for restoration.
    pub name: String,
    /// Capture completion status.
    pub status: BoatSavedSnapshotStatus,
    /// Exact source sandbox identity.
    pub source_sandbox_id: String,
}

/// Saved capture completion state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BoatSavedSnapshotStatus {
    /// Capture has not completed.
    Saving,
    /// Capture can be restored.
    Ready,
    /// Capture failed permanently.
    Failed,
}

/// Advisory size eligibility for the configured account.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoatLimits {
    /// Current size eligibility. This contains no full price catalogue.
    pub machine_types: Vec<BoatMachineType>,
}

/// One provider machine type eligibility record.
#[derive(Clone, Deserialize)]
pub struct BoatMachineType {
    /// Provider machine name.
    #[serde(rename = "type")]
    pub machine_type: String,
    /// Account eligibility, independent of current free capacity.
    pub available: bool,
    /// Provider explanation when unavailable.
    pub reason: Option<String>,
}
