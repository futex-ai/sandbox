//! Boundary envelopes for control operations.

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    error::{Error, Result},
    transport::request::BoatRoute,
};

use super::types::{BoatLabels, BoatSandbox, BoatSavedSnapshot};

#[derive(Deserialize)]
pub(super) struct SandboxEnvelope {
    pub ok: bool,
    #[serde(rename = "type")]
    pub kind: String,
    pub sandbox: BoatSandbox,
}

#[derive(Deserialize)]
pub(super) struct CreateEnvelope {
    pub ok: bool,
    #[serde(rename = "type")]
    pub kind: String,
    pub status: String,
    #[serde(rename = "ttlSeconds")]
    pub ttl_seconds: u32,
    pub sandbox: BoatSandbox,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ListEnvelope {
    pub ok: bool,
    #[serde(rename = "type")]
    pub kind: String,
    pub sandboxes: Vec<BoatSandbox>,
    pub page_info: Option<PageInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PageInfo {
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Deserialize)]
pub(super) struct SnapshotEnvelope {
    pub ok: bool,
    #[serde(rename = "type")]
    pub kind: String,
    pub snapshot: BoatSavedSnapshot,
}

#[derive(Deserialize)]
pub(super) struct SaveEnvelope {
    pub ok: bool,
    #[serde(rename = "type")]
    pub kind: String,
    pub status: String,
    pub snapshot: BoatSavedSnapshot,
}

#[derive(Deserialize)]
pub(super) struct SavedDeleted {
    pub ok: bool,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub status: String,
}

pub(super) fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    match serde_json::to_vec(value) {
        Ok(bytes) => Ok(bytes),
        Err(source) => Err(Error::internal_with(source, "encode Boat control request")),
    }
}

pub(super) fn decode<T: DeserializeOwned>(body: &[u8]) -> Result<T> {
    match serde_json::from_slice(body) {
        Ok(value) => Ok(value),
        Err(_) => Err(Error::InvalidResponse),
    }
}

pub(super) fn create_route() -> BoatRoute {
    BoatRoute::Sandboxes {
        labels: BoatLabels::new(),
        cursor: None,
    }
}
