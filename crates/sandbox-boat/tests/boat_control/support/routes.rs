//! Fake Boat API with snapshots-disabled capture rejection and exact deletion headers.

use serde_json::{Value, json};
use url::Url;

use super::{
    state::{State, error, sandbox},
    wire::{CapturedRequest, Reply},
};

pub(super) fn handle(state: &mut State, request: &CapturedRequest) -> Reply {
    if request.headers.get("authorization").map(String::as_str) != Some("Bearer fixture-key") {
        return error(401, "unauthorized");
    }
    if let Some(index) = state
        .overrides
        .iter()
        .position(|entry| entry.method == request.method && entry.path == request.path)
    {
        return state.overrides.remove(index).unwrap().reply;
    }
    let url = Url::parse(&format!("https://fixture{}", request.path)).unwrap();
    let path = url.path();
    if let Some(reply) = super::runtime_routes::handle(&mut state.runtime, request, &url) {
        return reply;
    }
    if path.starts_with("/api/v1/deletion-operations/") {
        return error(403, "organization_scope_forbidden");
    }
    match (request.method.as_str(), path) {
        ("POST", "/api/v1/sandboxes") => create(state, request),
        ("GET", "/api/v1/sandboxes") => list(state, &url),
        ("GET", "/api/v1/limits") => Reply::json(
            200,
            json!({"ok":true,"type":"limits.info","canStart":false,"machineTypes":state.machine_types}),
        ),
        ("POST", "/api/v1/saved-snapshots") => save(state, &request.body),
        _ => resource(state, request, path),
    }
}

fn create(state: &mut State, request: &CapturedRequest) -> Reply {
    let id = state.allocate_id();
    let mut value = sandbox(
        &id,
        request.body["labels"].clone(),
        request.body["snapshots"].as_bool().unwrap(),
    );
    value["type"] = request.body["type"].clone();
    value["ttlSeconds"] = request.body["ttlSeconds"].clone();
    value["monitoring"] = request.body["monitoring"].clone();
    state.sandboxes.insert(id, value.clone());
    state.create_reply.take().unwrap_or_else(|| Reply::json(202, json!({"ok":true,"type":"sandbox.created","status":"ready","ttlSeconds":request.body["ttlSeconds"],"sandbox":value})))
}

fn list(state: &State, url: &Url) -> Reply {
    let filters = url
        .query_pairs()
        .filter(|(key, _)| key == "label")
        .map(|(_, value)| {
            value
                .split_once('=')
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .unwrap()
        })
        .collect::<Vec<_>>();
    let rows = state
        .sandboxes
        .values()
        .filter(|row| {
            filters
                .iter()
                .all(|(key, value)| row["labels"][key].as_str() == Some(value))
        })
        .cloned()
        .collect::<Vec<_>>();
    let start = url
        .query_pairs()
        .find(|(key, _)| key == "cursor")
        .map_or(0, |(_, value)| value.parse::<usize>().unwrap());
    let end = (start + state.page_size).min(rows.len());
    let more = end < rows.len();
    Reply::json(
        200,
        json!({"ok":true,"type":"sandbox.list","sandboxes":&rows[start..end],
        "pageInfo":{"hasMore":more,"nextCursor":more.then(|| end.to_string()),"limit":200}}),
    )
}

fn save(state: &mut State, body: &Value) -> Reply {
    let source = body["sandboxId"].as_str().unwrap();
    let Some(row) = state.sandboxes.get(source) else {
        return error(404, "not_found");
    };
    if row["snapshots"] != true {
        return error(409, "snapshots_disabled");
    }
    let name = body["name"].as_str().unwrap();
    let snapshot = json!({"name":name,"status":state.snapshot_status,"sourceSandboxId":source,"createdAt":"2026-11-04T00:00:00Z"});
    state.snapshots.insert(name.to_owned(), snapshot.clone());
    state.save_reply.take().unwrap_or_else(|| {
        Reply::json(
            202,
            json!({"ok":true,"type":"snapshot.saved.saving","status":"saving","snapshot":snapshot}),
        )
    })
}

fn resource(state: &mut State, request: &CapturedRequest, path: &str) -> Reply {
    if let Some(id) = path.strip_prefix("/api/v1/sandboxes/") {
        return match request.method.as_str() {
            "GET" => state.sandboxes.get(id).map_or_else(
                || error(404, "not_found"),
                |row| Reply::json(200, json!({"ok":true,"type":"sandbox.info","sandbox":row})),
            ),
            "DELETE" => delete(state, request, id),
            _ => error(400, "invalid_request"),
        };
    }
    if let Some(name) = path.strip_prefix("/api/v1/saved-snapshots/") {
        return match request.method.as_str() {
            "GET" => state.snapshots.get(name).map_or_else(
                || error(404, "not_found"),
                |row| {
                    Reply::json(
                        200,
                        json!({"ok":true,"type":"snapshot.saved.info","snapshot":row}),
                    )
                },
            ),
            "DELETE" => {
                if state.snapshots.remove(name).is_none() {
                    return error(404, "not_found");
                }
                Reply::json(
                    200,
                    json!({"ok":true,"type":"snapshot.saved.deleted","status":"deleted","name":name}),
                )
            }
            _ => error(400, "invalid_request"),
        };
    }
    error(404, "unknown_route")
}

fn delete(state: &mut State, request: &CapturedRequest, id: &str) -> Reply {
    if request
        .headers
        .get("x-ascii-confirm-delete")
        .map(String::as_str)
        != Some(id)
    {
        return error(409, "confirmation_required");
    }
    if !state.sandboxes.contains_key(id) {
        return error(404, "not_found");
    }
    let status = state
        .deletion_statuses
        .pop_front()
        .unwrap_or_else(|| "completed".to_owned());
    if status == "completed" {
        state.sandboxes.remove(id);
    }
    Reply::json(
        202,
        json!({"ok":true,"type":"deletion.operation","operation":{
        "id":"bdop_0123456789abcdef0123456789abcdef","kind":"sandbox","targetId":id,"status":status}}),
    )
}
