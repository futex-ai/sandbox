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
