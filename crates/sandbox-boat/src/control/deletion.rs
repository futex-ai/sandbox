//! Sandbox deletion through repeated scoped DELETE responses only.

use std::time::Duration;

use serde::Deserialize;

use crate::{
    error::{Error, Result},
    transport::request::{BoatHttpMethod, BoatHttpRequest, BoatRoute},
};

use super::{acknowledgment, client::ReqwestBoatControlApi, payload};

pub(super) async fn delete(client: &ReqwestBoatControlApi, id: &str) -> Result<()> {
    match tokio::time::timeout(Duration::from_secs(60), delete_until_complete(client, id)).await {
        Ok(result) => result,
        Err(_) => Err(Error::Unavailable),
    }
}

async fn delete_until_complete(client: &ReqwestBoatControlApi, id: &str) -> Result<()> {
    let mut operation_id = None;
    loop {
        let response = client
            .transport
            .send(BoatHttpRequest {
                method: BoatHttpMethod::Delete,
                route: BoatRoute::Sandbox(id.to_owned()),
                body: None,
                idempotency_key: None,
            })
            .await?;
        if response.status == 404 {
            return Ok(());
        }
        if !matches!(response.status, 200 | 202) {
            return Err(acknowledgment::read_status(response.status));
        }
        let envelope: DeletionEnvelope = payload::decode(&response.body)?;
        let operation = envelope.operation;
        if !envelope.ok
            || !matches!(
                envelope.kind.as_str(),
                "deletion.operation" | "sandbox.deleting"
            )
            || operation.kind != "sandbox"
            || operation.target_id != id
            || !valid_operation_id(&operation.id)
            || operation_id
                .as_ref()
                .is_some_and(|expected| expected != &operation.id)
        {
            return Err(Error::InvalidResponse);
        }
        operation_id = Some(operation.id);
        match operation.status {
            DeletionStatus::Completed => return Ok(()),
            DeletionStatus::Pending | DeletionStatus::Processing | DeletionStatus::Blocked => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

#[derive(Deserialize)]
struct DeletionEnvelope {
    ok: bool,
    #[serde(rename = "type")]
    kind: String,
    operation: DeletionOperation,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeletionOperation {
    id: String,
    kind: String,
    target_id: String,
    status: DeletionStatus,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum DeletionStatus {
    Pending,
    Processing,
    Blocked,
    Completed,
}

fn valid_operation_id(value: &str) -> bool {
    value.len() == 37
        && value.starts_with("bdop_")
        && value.as_bytes()[5..]
            .iter()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
