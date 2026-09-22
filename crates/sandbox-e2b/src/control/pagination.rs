//! Bounded cursor traversal for provider inventory APIs.

use std::collections::HashSet;

use crate::error::{Error, Result};

use super::{
    ReqwestE2bControlApi,
    helpers::{map_listed_sandbox, metadata_query, valid_provider_identity},
    http::Method,
    types::{
        ControlSandbox, ControlSnapshot, ListedSandboxBody, SandboxMetadata, SnapshotInfoBody,
    },
};

const PAGE_SIZE: &str = "100";
const MAX_INVENTORY_PAGES: usize = 1_000;

pub(super) async fn list_sandboxes(
    client: &ReqwestE2bControlApi,
    metadata: SandboxMetadata,
) -> Result<Vec<ControlSandbox>> {
    let metadata = metadata_query(&metadata);
    let mut pagination = Pagination::default();
    let mut result = Vec::new();
    loop {
        let path = {
            let mut query = url::form_urlencoded::Serializer::new(String::new());
            query.append_pair("metadata", &metadata);
            query.append_pair("limit", PAGE_SIZE);
            pagination.append(&mut query);
            format!("/v2/sandboxes?{}", query.finish())
        };
        let (rows, next): (Vec<ListedSandboxBody>, _) = client
            .json_page(Method::Get, path, None, &[200], false)
            .await?;
        for row in rows {
            if !valid_provider_identity(&row.sandbox_id) {
                return Err(Error::Unavailable);
            }
            result.push(map_listed_sandbox(row));
        }
        if !pagination.advance(next)? {
            return Ok(result);
        }
    }
}

pub(super) async fn list_snapshots(
    client: &ReqwestE2bControlApi,
    sandbox_id: &str,
    name: &str,
) -> Result<Vec<ControlSnapshot>> {
    if sandbox_id.is_empty() || name.is_empty() {
        return Err(Error::InvalidRequest);
    }
    let mut pagination = Pagination::default();
    let mut result = Vec::new();
    loop {
        let path = {
            let mut query = url::form_urlencoded::Serializer::new(String::new());
            query.append_pair("sandboxID", sandbox_id);
            query.append_pair("limit", PAGE_SIZE);
            pagination.append(&mut query);
            format!("/snapshots?{}", query.finish())
        };
        let (rows, next): (Vec<SnapshotInfoBody>, _) = client
            .json_page(Method::Get, path, None, &[200], false)
            .await?;
        for row in rows {
            if !valid_provider_identity(&row.snapshot_id) {
                return Err(Error::Unavailable);
            }
            if snapshot_has_name(&row, name) {
                result.push(ControlSnapshot {
                    snapshot_id: row.snapshot_id,
                });
            }
        }
        if !pagination.advance(next)? {
            return Ok(result);
        }
    }
}

#[derive(Default)]
struct Pagination {
    next_token: Option<String>,
    seen: HashSet<String>,
    pages: usize,
}

impl Pagination {
    fn append(&self, query: &mut url::form_urlencoded::Serializer<'_, String>) {
        if let Some(next_token) = &self.next_token {
            query.append_pair("nextToken", next_token);
        }
    }

    fn advance(&mut self, next_token: Option<String>) -> Result<bool> {
        self.pages += 1;
        let Some(next_token) = next_token else {
            return Ok(false);
        };
        if self.pages >= MAX_INVENTORY_PAGES || !self.seen.insert(next_token.clone()) {
            return Err(Error::InvalidPagination);
        }
        self.next_token = Some(next_token);
        Ok(true)
    }
}

fn snapshot_has_name(snapshot: &SnapshotInfoBody, expected: &str) -> bool {
    snapshot.names.iter().any(|name| {
        let leaf = match name.rsplit('/').next() {
            Some(leaf) => leaf,
            None => name,
        };
        leaf.split(':').next() == Some(expected)
    })
}
