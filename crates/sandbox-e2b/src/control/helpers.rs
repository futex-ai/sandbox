//! Pure control API encoding, status, and mapping helpers.

use serde::Serialize;

use crate::error::{Error, Result};

use super::types::{
    ControlSandbox, ControlSandboxAccess, ControlSandboxState, ListedSandboxBody,
    SandboxAccessBody, SandboxMetadata, SandboxStateBody,
};

pub(super) fn encode(value: &impl Serialize) -> Result<Vec<u8>> {
    match serde_json::to_vec(value) {
        Ok(value) => Ok(value),
        Err(source) => Err(Error::internal_with(source, "encode E2B control request")),
    }
}

pub(super) fn ensure_status(status: u16, accepted: &[u16], ambiguous: bool) -> Result<()> {
    if accepted.contains(&status) {
        return Ok(());
    }
    match status {
        400 | 409 | 422 => Err(Error::InvalidRequest),
        401 | 403 => Err(Error::Unauthorized),
        404 => Err(Error::NotFound),
        408 | 429 | 500..=599 if ambiguous => Err(Error::DeliveryAmbiguous),
        408 | 429 | 500..=599 => Err(Error::Unavailable),
        _ => Err(Error::InvalidRequest),
    }
}

pub(super) fn map_access(
    body: SandboxAccessBody,
    sandbox_domain: &str,
) -> Result<ControlSandboxAccess> {
    let token = body
        .envd_access_token
        .filter(|token| !token.trim().is_empty())
        .ok_or(Error::Unavailable)?;
    let traffic_token = body
        .traffic_access_token
        .filter(|token| !token.trim().is_empty())
        .ok_or(Error::Unavailable)?;
    Ok(ControlSandboxAccess {
        sandbox_id: body.sandbox_id,
        domain: sandbox_domain.to_owned(),
        envd_access_token: token,
        traffic_access_token: Some(traffic_token),
    })
}

pub(super) fn ensure_sandbox_identity(expected: &str, actual: &str) -> Result<()> {
    if expected != actual {
        return Err(Error::internal_message(
            "E2B existing sandbox response identity mismatch",
        ));
    }
    Ok(())
}

pub(super) fn map_listed_sandbox(body: ListedSandboxBody) -> ControlSandbox {
    ControlSandbox {
        sandbox_id: body.sandbox_id,
        state: map_state(body.state),
        metadata: body.metadata,
    }
}

pub(super) fn map_state(state: SandboxStateBody) -> ControlSandboxState {
    match state {
        SandboxStateBody::Running => ControlSandboxState::Running,
        SandboxStateBody::Paused => ControlSandboxState::Paused,
    }
}

pub(super) fn metadata_query(metadata: &SandboxMetadata) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.extend_pairs(metadata);
    serializer.finish()
}

pub(super) fn path_segment(value: &str) -> Result<String> {
    if !valid_provider_identity(value) {
        return Err(Error::InvalidRequest);
    }
    Ok(url::form_urlencoded::byte_serialize(value.as_bytes()).collect())
}

/// Returns whether an opaque provider identity remains a usable route segment.
pub(super) fn valid_provider_identity(value: &str) -> bool {
    !matches!(value, "" | "." | "..")
}

/// Returns whether a sandbox identity fits the DNS label used by envd routes.
pub(super) fn valid_sandbox_identity(value: &str) -> bool {
    const DNS_LABEL_MAX_BYTES: usize = 63;
    const ROUTING_PREFIX_BYTES: usize = "65535-".len();
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= DNS_LABEL_MAX_BYTES - ROUTING_PREFIX_BYTES
        && bytes
            .first()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

#[cfg(test)]
#[path = "_tests_/control_helper_tests.rs"]
mod control_helper_tests;
