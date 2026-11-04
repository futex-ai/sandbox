//! Typed HTTP routes and metadata-only request diagnostics.

use std::{collections::BTreeMap, fmt};

use crate::error::{Error, Result};

/// HTTP methods supported by the Boat control API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoatHttpMethod {
    /// Non-mutating resource read.
    Get,
    /// Initial resource mutation.
    Post,
    /// Idempotent resource deletion.
    Delete,
}

/// Routes relative to the fixed, validated `/api/v1` base.
#[derive(Clone, Eq, PartialEq)]
pub enum BoatRoute {
    /// Sandbox create or filtered paginated inventory.
    Sandboxes {
        /// Required provider label matches.
        labels: BTreeMap<String, String>,
        /// Opaque inventory cursor, encoded as a query value.
        cursor: Option<String>,
    },
    /// Exact sandbox identity.
    Sandbox(String),
    /// Initial saved-snapshot creation.
    SavedSnapshots,
    /// Exact saved-snapshot name.
    SavedSnapshot(String),
    /// Advisory account capacity.
    Limits,
}

/// One provider request. Diagnostics omit its body and route identifiers.
#[derive(Clone)]
pub struct BoatHttpRequest {
    /// Validated operation method.
    pub method: BoatHttpMethod,
    /// Typed route validated before credentials are attached.
    pub route: BoatRoute,
    /// Serialized boundary payload, when required.
    pub body: Option<Vec<u8>>,
    /// Durable create identity sent as `Idempotency-Key`.
    pub idempotency_key: Option<String>,
}

impl fmt::Debug for BoatHttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoatHttpRequest")
            .field("method", &self.method)
            .field("body_bytes", &self.body.as_ref().map(Vec::len))
            .field("has_idempotency_key", &self.idempotency_key.is_some())
            .finish()
    }
}

impl BoatRoute {
    pub(crate) fn path(&self, method: BoatHttpMethod) -> Result<String> {
        let path = match (self, method) {
            (Self::Sandboxes { labels, cursor }, BoatHttpMethod::Get) => {
                let mut query = url::form_urlencoded::Serializer::new(String::new());
                query.append_pair("limit", "200").append_pair("sort", "asc");
                for (key, value) in labels {
                    query.append_pair("label", &format!("{key}={value}"));
                }
                if let Some(cursor) = cursor {
                    query.append_pair("cursor", cursor);
                }
                format!("/sandboxes?{}", query.finish())
            }
            (Self::Sandboxes { labels, cursor }, BoatHttpMethod::Post)
                if labels.is_empty() && cursor.is_none() =>
            {
                "/sandboxes".to_owned()
            }
            (Self::Sandbox(id), BoatHttpMethod::Get | BoatHttpMethod::Delete)
                if valid_sandbox_id(id) =>
            {
                format!("/sandboxes/{id}")
            }
            (Self::SavedSnapshots, BoatHttpMethod::Post) => "/saved-snapshots".to_owned(),
            (Self::SavedSnapshot(name), BoatHttpMethod::Get | BoatHttpMethod::Delete)
                if valid_snapshot_name(name) =>
            {
                format!("/saved-snapshots/{name}")
            }
            (Self::Limits, BoatHttpMethod::Get) => "/limits".to_owned(),
            _ => return Err(Error::InvalidRequest),
        };
        Ok(path)
    }
}

pub(crate) fn valid_sandbox_id(id: &str) -> bool {
    id.len() == 11
        && id.starts_with("bx_")
        && id.as_bytes()[3..]
            .iter()
            .all(|byte| b"23456789abcdefghjkmnpqrstuvwxyz".contains(byte))
}

pub(crate) fn valid_snapshot_name(name: &str) -> bool {
    (1..=63).contains(&name.len())
        && name.as_bytes()[0].is_ascii_alphanumeric()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}
