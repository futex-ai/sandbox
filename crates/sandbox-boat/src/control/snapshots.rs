//! Exact-name saved snapshots without sandbox deletion assumptions.

use crate::{
    error::{Error, Result},
    transport::request::{
        BoatHttpMethod, BoatHttpRequest, BoatRoute, valid_sandbox_id, valid_snapshot_name,
    },
};

use super::{
    acknowledgment,
    client::ReqwestBoatControlApi,
    payload::{self, SaveEnvelope, SavedDeleted, SnapshotEnvelope},
    types::{BoatSaveSnapshot, BoatSavedSnapshot},
};

pub(super) async fn get(client: &ReqwestBoatControlApi, name: &str) -> Result<BoatSavedSnapshot> {
    let response = client
        .transport
        .send(BoatHttpRequest {
            method: BoatHttpMethod::Get,
            route: BoatRoute::SavedSnapshot(name.to_owned()),
            body: None,
            idempotency_key: None,
        })
        .await?;
    if response.status != 200 {
        return Err(acknowledgment::read_status(response.status));
    }
    let envelope: SnapshotEnvelope = payload::decode(&response.body)?;
    acknowledgment::snapshot(&envelope.snapshot, name)?;
    if !envelope.ok || envelope.kind != "snapshot.saved.info" {
        return Err(Error::InvalidResponse);
    }
    Ok(envelope.snapshot)
}

pub(super) async fn save(
    client: &ReqwestBoatControlApi,
    request: BoatSaveSnapshot,
) -> Result<BoatSavedSnapshot> {
    if !valid_sandbox_id(&request.sandbox_id)
        || !valid_snapshot_name(&request.name)
        || matches!(
            request.name.as_str(),
            "latest" | "tree" | "pull" | "rm" | "save" | "current" | "self" | "new"
        )
    {
        return Err(Error::InvalidRequest);
    }
    let response = client
        .transport
        .send(BoatHttpRequest {
            method: BoatHttpMethod::Post,
            route: BoatRoute::SavedSnapshots,
            body: Some(payload::encode(&request)?),
            idempotency_key: None,
        })
        .await?;
    if response.status != 202 {
        return Err(
            if matches!(response.status, 400 | 401 | 402 | 403 | 404 | 409) {
                acknowledgment::read_status(response.status)
            } else {
                Error::DeliveryAmbiguous
            },
        );
    }
    let envelope: SaveEnvelope = match payload::decode(&response.body) {
        Ok(value) => value,
        Err(_) => return Err(Error::DeliveryAmbiguous),
    };
    if !envelope.ok
        || envelope.kind != "snapshot.saved.saving"
        || envelope.status != "saving"
        || acknowledgment::snapshot(&envelope.snapshot, &request.name).is_err()
        || envelope.snapshot.source_sandbox_id != request.sandbox_id
    {
        return Err(Error::DeliveryAmbiguous);
    }
    Ok(envelope.snapshot)
}

pub(super) async fn delete(client: &ReqwestBoatControlApi, name: &str) -> Result<()> {
    let response = client
        .transport
        .send(BoatHttpRequest {
            method: BoatHttpMethod::Delete,
            route: BoatRoute::SavedSnapshot(name.to_owned()),
            body: None,
            idempotency_key: None,
        })
        .await?;
    if response.status == 404 {
        return Ok(());
    }
    if response.status != 200 {
        return Err(acknowledgment::read_status(response.status));
    }
    let envelope: SavedDeleted = payload::decode(&response.body)?;
    if !envelope.ok
        || envelope.kind != "snapshot.saved.deleted"
        || envelope.name != name
        || envelope.status != "deleted"
    {
        return Err(Error::InvalidResponse);
    }
    Ok(())
}
