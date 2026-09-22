//! Tests for the lifecycle-invisible E2B read-access request.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use unimock::{MockFn, Unimock, matching};

use crate::{
    E2bAdapterError, E2bControlApi,
    control::http::{E2bHttpTransport, HttpRequest, HttpResponse, Method, send},
};

use super::ReqwestE2bControlApi;

type RecordedRequests = Arc<Mutex<Vec<HttpRequest>>>;

#[tokio::test]
async fn read_access_uses_get_without_connect_or_timeout_mutation() {
    let (client, requests) = recording_client(vec![json_response(
        200,
        r#"{
            "sandboxID":"provider-sandbox",
            "state":"running",
            "envdAccessToken":"token",
            "lifecycle":{"autoResume":false}
        }"#,
    )]);

    let access = client
        .get_sandbox_read_access("provider-sandbox")
        .await
        .expect("running non-auto-resuming sandbox");

    assert_eq!(access.sandbox_id, "provider-sandbox");
    let request = only_request(&requests);
    assert_eq!(request.method, Method::Get);
    assert_eq!(request.path_and_query, "/sandboxes/provider-sandbox");
    assert!(request.body.is_none());
}

#[tokio::test]
async fn one_shot_connect_uses_only_non_mutating_read_access() {
    let (client, requests) = recording_client(vec![json_response(
        200,
        r#"{
            "sandboxID":"provider-sandbox",
            "state":"running",
            "envdAccessToken":"token",
            "metadata":{"sandbox_lifetime":"one_shot"},
            "lifecycle":{"autoResume":false}
        }"#,
    )]);

    let access = client
        .connect_sandbox("provider-sandbox")
        .await
        .expect("running one-shot sandbox should expose read access");

    assert_eq!(access.envd_access_token, "token");
    assert_eq!(access.traffic_access_token, None);
    let requests = requests.lock().expect("request lock");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, Method::Get);
    assert_eq!(requests[0].path_and_query, "/sandboxes/provider-sandbox");
    assert!(requests[0].body.is_none());
}

#[tokio::test]
async fn one_shot_pause_is_rejected_without_a_mutating_request() {
    let (client, requests) = recording_client(vec![json_response(
        200,
        r#"{
            "sandboxID":"provider-sandbox",
            "state":"running",
            "metadata":{"sandbox_lifetime":"one_shot"}
        }"#,
    )]);

    assert!(matches!(
        client.pause_sandbox("provider-sandbox").await,
        Err(E2bAdapterError::Unavailable)
    ));

    let requests = requests.lock().expect("request lock");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, Method::Get);
}

#[tokio::test]
async fn legacy_connect_keeps_the_configured_idle_timeout() {
    let (client, requests) = recording_client(vec![
        json_response(200, r#"{"sandboxID":"provider-sandbox","state":"paused"}"#),
        json_response(
            201,
            r#"{"sandboxID":"provider-sandbox","envdAccessToken":"envd","trafficAccessToken":"traffic"}"#,
        ),
    ]);

    let access = client
        .connect_sandbox("provider-sandbox")
        .await
        .expect("legacy sandbox should use ordinary connect");

    assert_eq!(access.traffic_access_token.as_deref(), Some("traffic"));
    let requests = requests.lock().expect("request lock");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, Method::Get);
    assert_eq!(requests[1].method, Method::Post);
    let body: serde_json::Value =
        serde_json::from_slice(requests[1].body.as_deref().expect("connect body"))
            .expect("connect body should be JSON");
    assert_eq!(body, serde_json::json!({"timeout": 600}));
}

#[tokio::test]
async fn read_access_rejects_paused_or_auto_resuming_sandboxes() {
    let (client, _) = recording_client(vec![
        json_response(
            200,
            r#"{
                "sandboxID":"paused",
                "state":"paused",
                "envdAccessToken":"token",
                "lifecycle":{"autoResume":false}
            }"#,
        ),
        json_response(
            200,
            r#"{
                "sandboxID":"auto",
                "state":"running",
                "envdAccessToken":"token",
                "lifecycle":{"autoResume":true}
            }"#,
        ),
    ]);

    assert!(matches!(
        client.get_sandbox_read_access("paused").await,
        Err(E2bAdapterError::Unavailable)
    ));
    assert!(matches!(
        client.get_sandbox_read_access("auto").await,
        Err(E2bAdapterError::Unavailable)
    ));
}

#[tokio::test]
async fn read_access_rejects_missing_or_empty_credentials_as_unavailable() {
    let (client, _) = recording_client(vec![
        json_response(
            200,
            r#"{
                "sandboxID":"missing",
                "state":"running",
                "lifecycle":{"autoResume":false}
            }"#,
        ),
        json_response(
            200,
            r#"{
                "sandboxID":"empty",
                "state":"running",
                "envdAccessToken":"",
                "lifecycle":{"autoResume":false}
            }"#,
        ),
    ]);

    for sandbox_id in ["missing", "empty"] {
        assert!(matches!(
            client.get_sandbox_read_access(sandbox_id).await,
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
        ReqwestE2bControlApi {
            transport,
            sandbox_domain: "e2b.app".to_owned(),
            idle_timeout_seconds: 600,
            lifetime_metadata_key: "sandbox_lifetime".to_owned(),
        },
        requests,
    )
}

fn json_response(status: u16, body: &str) -> HttpResponse {
    HttpResponse {
        status,
        body: body.as_bytes().to_vec(),
        next_token: None,
    }
}

fn only_request(requests: &RecordedRequests) -> HttpRequest {
    requests.lock().expect("request lock")[0].clone()
}
