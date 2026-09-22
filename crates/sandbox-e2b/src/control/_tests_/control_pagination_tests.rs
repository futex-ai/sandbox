//! Cursor pagination tests for provider inventory and snapshot lookup.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use unimock::{MockFn, Unimock, matching};

use crate::{E2bAdapterError, E2bControlApi, SandboxMetadata};

use super::ReqwestE2bControlApi;
use crate::control::http::{E2bHttpTransport, HttpRequest, HttpResponse, send};

type RecordedRequests = Arc<Mutex<Vec<HttpRequest>>>;

impl ReqwestE2bControlApi {
    fn with_pagination_transport(transport: Arc<dyn E2bHttpTransport>) -> Self {
        Self {
            transport,
            sandbox_domain: "e2b.app".to_owned(),
            idle_timeout_seconds: 600,
            lifetime_metadata_key: "sandbox_lifetime".to_owned(),
        }
    }
}

#[tokio::test]
async fn sandbox_inventory_consumes_every_cursor_page() {
    let (client, requests) = recording_client(vec![
        page(
            r#"[{"sandboxID":"sandbox-one","state":"running"}]"#,
            Some("cursor/one"),
        ),
        page(r#"[{"sandboxID":"sandbox-two","state":"paused"}]"#, None),
    ]);

    let rows = client
        .list_sandboxes(SandboxMetadata::new())
        .await
        .expect("all sandbox pages should be returned");

    assert_eq!(rows.len(), 2);
    assert_eq!(
        query_value(&requests, 1, "nextToken").as_deref(),
        Some("cursor/one")
    );
}

#[tokio::test]
async fn snapshot_inventory_and_lookup_consume_every_cursor_page() {
    let first = r#"[{"snapshotID":"team/first:one","names":["team/other:one"]}]"#;
    let target = r#"[{"snapshotID":"team/target:one","names":["team/sandbox-operation:one"]}]"#;
    let (client, requests) = recording_client(vec![
        page(first, Some("list-cursor")),
        page(target, None),
        page(first, Some("get-cursor")),
        page(target, None),
    ]);

    let listed = client
        .list_snapshots("source", "sandbox-operation")
        .await
        .expect("all matching snapshot pages should be returned");
    let found = client
        .get_snapshot("source", "sandbox-operation", "team/target:one")
        .await
        .expect("snapshot lookup should search filtered later pages");

    assert_eq!(listed.len(), 1);
    assert_eq!(found.snapshot_id, "team/target:one");
    assert_eq!(
        query_value(&requests, 1, "nextToken").as_deref(),
        Some("list-cursor")
    );
    assert_eq!(
        query_value(&requests, 3, "nextToken").as_deref(),
        Some("get-cursor")
    );
    assert_eq!(
        query_value(&requests, 2, "sandboxID").as_deref(),
        Some("source")
    );
    assert_eq!(
        query_value(&requests, 3, "sandboxID").as_deref(),
        Some("source")
    );
}

#[tokio::test]
async fn repeated_inventory_cursor_is_rejected() {
    let (client, _) = recording_client(vec![
        page("[]", Some("repeated")),
        page("[]", Some("repeated")),
    ]);

    assert!(matches!(
        client.list_sandboxes(SandboxMetadata::new()).await,
        Err(E2bAdapterError::InvalidPagination)
    ));
}

#[tokio::test]
async fn snapshot_inventory_rejects_unusable_provider_identities() {
    for snapshot_id in ["", ".", ".."] {
        let body =
            format!(r#"[{{"snapshotID":"{snapshot_id}","names":["team/sandbox-operation:one"]}}]"#);
        let (client, _) = recording_client(vec![page(&body, None)]);

        assert!(matches!(
            client.list_snapshots("source", "sandbox-operation").await,
            Err(E2bAdapterError::Unavailable)
        ));
    }
}

fn recording_client(responses: Vec<HttpResponse>) -> (ReqwestE2bControlApi, RecordedRequests) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let responses = Arc::new(Mutex::new(VecDeque::from(responses)));
    let transport: Arc<dyn E2bHttpTransport> =
        Arc::new(Unimock::new(send.each_call(matching!(_)).answers_arc({
            let requests = requests.clone();
            let responses = responses.clone();
            Arc::new(move |_, request: HttpRequest| {
                requests.lock().expect("request lock").push(request);
                Ok(responses
                    .lock()
                    .expect("response lock")
                    .pop_front()
                    .expect("unexpected control request"))
            })
        })));
    (
        ReqwestE2bControlApi::with_pagination_transport(transport),
        requests,
    )
}

fn page(body: &str, next_token: Option<&str>) -> HttpResponse {
    HttpResponse {
        status: 200,
        body: body.as_bytes().to_vec(),
        next_token: next_token.map(str::to_owned),
    }
}

fn query_value(requests: &RecordedRequests, index: usize, key: &str) -> Option<String> {
    let request = requests.lock().expect("request lock")[index].clone();
    let query = request.path_and_query.split_once('?')?.1;
    let values = url::form_urlencoded::parse(query.as_bytes()).collect::<HashMap<_, _>>();
    values.get(key).map(ToString::to_string)
}
