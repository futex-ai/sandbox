//! Direct sandbox dispatch and bounded read-only inventory.

use std::{collections::HashSet, time::Duration};

use crate::{
    catalogue,
    config::canonical,
    error::{Error, Result},
    transport::request::{BoatHttpMethod, BoatHttpRequest, BoatRoute, valid_snapshot_name},
};

use super::{
    acknowledgment,
    client::ReqwestBoatControlApi,
    payload::{self, CreateEnvelope, ListEnvelope, SandboxEnvelope},
    refusal,
    types::{BoatCreateSandbox, BoatLabels, BoatSandbox},
};

pub(super) async fn create(
    client: &ReqwestBoatControlApi,
    request: BoatCreateSandbox,
) -> Result<BoatSandbox> {
    validate_create(&request)?;
    let response = client
        .transport
        .send(BoatHttpRequest {
            method: BoatHttpMethod::Post,
            route: payload::create_route(),
            body: Some(payload::encode(&request)?),
            idempotency_key: Some(request.operation.clone()),
        })
        .await?;
    if response.status != 202 {
        if let Some(refusal) = refusal::start_refusal(&response) {
            return Err(Error::StartRefused { refusal });
        }
        return Err(Error::DeliveryAmbiguous);
    }
    let created: CreateEnvelope = match payload::decode(&response.body) {
        Ok(created) => created,
        Err(_) => return Err(Error::DeliveryAmbiguous),
    };
    if !created.ok
        || created.kind != "sandbox.created"
        || !matches!(created.status.as_str(), "provisioning" | "ready")
        || created.ttl_seconds != request.ttl_seconds
        || acknowledgment::sandbox(&created.sandbox).is_err()
        || created
            .sandbox
            .machine_type
            .as_ref()
            .is_some_and(|value| value != &request.machine_type)
        || created
            .sandbox
            .ttl_seconds
            .is_some_and(|value| value != request.ttl_seconds)
        || created.sandbox.snapshots == Some(true)
        || created.sandbox.monitoring == Some(true)
        || (!created.sandbox.labels.is_empty()
            && request
                .labels
                .iter()
                .any(|(key, expected)| created.sandbox.labels.get(key) != Some(expected)))
    {
        return Err(Error::DeliveryAmbiguous);
    }
    Ok(created.sandbox)
}

pub(super) async fn get(client: &ReqwestBoatControlApi, id: &str) -> Result<BoatSandbox> {
    let response = client
        .transport
        .send(BoatHttpRequest {
            method: BoatHttpMethod::Get,
            route: BoatRoute::Sandbox(id.to_owned()),
            body: None,
            idempotency_key: None,
        })
        .await?;
    if response.status != 200 {
        return Err(acknowledgment::read_status(response.status));
    }
    let envelope: SandboxEnvelope = payload::decode(&response.body)?;
    acknowledgment::sandbox(&envelope.sandbox)?;
    if !envelope.ok || envelope.kind != "sandbox.info" || envelope.sandbox.id != id {
        return Err(Error::InvalidResponse);
    }
    Ok(envelope.sandbox)
}

pub(super) async fn list(
    client: &ReqwestBoatControlApi,
    labels: BoatLabels,
) -> Result<Vec<BoatSandbox>> {
    match tokio::time::timeout(Duration::from_secs(60), list_pages(client, labels)).await {
        Ok(result) => result,
        Err(_) => Err(Error::Unavailable),
    }
}

async fn list_pages(
    client: &ReqwestBoatControlApi,
    labels: BoatLabels,
) -> Result<Vec<BoatSandbox>> {
    let mut rows = Vec::new();
    let mut cursor = None;
    let mut cursors = HashSet::new();
    let mut ids = HashSet::new();
    for _ in 0..100 {
        let response = client
            .transport
            .send(BoatHttpRequest {
                method: BoatHttpMethod::Get,
                route: BoatRoute::Sandboxes {
                    labels: labels.clone(),
                    cursor,
                },
                body: None,
                idempotency_key: None,
            })
            .await?;
        if response.status != 200 {
            return Err(acknowledgment::read_status(response.status));
        }
        let envelope: ListEnvelope = payload::decode(&response.body)?;
        if !envelope.ok || envelope.kind != "sandbox.list" || envelope.sandboxes.len() > 200 {
            return Err(Error::InvalidResponse);
        }
        for sandbox in envelope.sandboxes {
            acknowledgment::sandbox(&sandbox)?;
            if !ids.insert(sandbox.id.clone()) {
                return Err(Error::InvalidPagination);
            }
            rows.push(sandbox);
        }
        let Some(page) = envelope.page_info else {
            return Ok(rows);
        };
        if !page.has_more {
            if page.next_cursor.is_some() {
                return Err(Error::InvalidPagination);
            }
            return Ok(rows);
        }
        let next = page.next_cursor.ok_or(Error::InvalidPagination)?;
        if !canonical(&next) || next.len() > 4096 || !cursors.insert(next.clone()) {
            return Err(Error::InvalidPagination);
        }
        cursor = Some(next);
    }
    Err(Error::InvalidPagination)
}

fn validate_create(request: &BoatCreateSandbox) -> Result<()> {
    if catalogue::offer(&request.machine_type).is_none()
        || !(1..=2_592_000).contains(&request.ttl_seconds)
        || !request.no_env
        || request.snapshots
        || request.monitoring
        || request
            .from
            .as_deref()
            .is_some_and(|name| !valid_snapshot_name(name))
        || !canonical(&request.operation)
        || request.operation.len() > 128
        || !request
            .operation
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || request.labels.len() > 20
        || request.labels.iter().any(|(key, value)| {
            key.is_empty()
                || key.len() > 63
                || !key.as_bytes()[0].is_ascii_alphanumeric()
                || !key
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
                || value.len() > 256
        })
    {
        return Err(Error::InvalidRequest);
    }
    Ok(())
}
