//! Unit tests for the typed E2B control client.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use serde_json::Value;
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlCreateSandbox, E2bAdapterConfig, E2bAdapterError, E2bControlApi, E2bProfile,
    SandboxMetadata,
    control::{ControlSandboxAccess, ControlSandboxState},
};

use super::ReqwestE2bControlApi;
use crate::control::http::{E2bHttpTransport, HttpRequest, HttpResponse, Method, send};

type RecordedRequests = Arc<Mutex<Vec<HttpRequest>>>;

impl ReqwestE2bControlApi {
    fn with_transport(
        transport: Arc<dyn E2bHttpTransport>,
        sandbox_domain: impl Into<String>,
        idle_timeout_seconds: u32,
    ) -> Self {
        Self {
            transport,
            sandbox_domain: sandbox_domain.into(),
            idle_timeout_seconds,
        }
    }
}

#[tokio::test]
async fn list_sandboxes_uses_v2_and_escapes_metadata_once() {
    let (client, requests) = recording_client(vec![json_response(
        200,
        r#"[{"sandboxID":"provider-sandbox","state":"paused"}]"#,
    )]);
    let metadata = SandboxMetadata::from([
        ("agent key".to_owned(), "agent/value".to_owned()),
        ("operation".to_owned(), "a&b".to_owned()),
    ]);

    let listed = client
        .list_sandboxes(metadata)
        .await
        .expect("list should decode");

    assert_eq!(listed[0].state, ControlSandboxState::Paused);
    let request = only_request(&requests);
    assert_eq!(request.method, Method::Get);
    let query = request
        .path_and_query
        .split_once('?')
        .expect("query should be present")
        .1;
    let query = url::form_urlencoded::parse(query.as_bytes()).collect::<HashMap<_, _>>();
    assert_eq!(query.get("limit").map(AsRef::as_ref), Some("100"));
    assert_eq!(
        query.get("metadata").map(AsRef::as_ref),
        Some("agent+key=agent%2Fvalue&operation=a%26b")
    );
}

#[tokio::test]
async fn create_sandbox_applies_secure_private_defaults() {
    let (client, requests) = recording_client(vec![json_response(
        201,
        r#"{"sandboxID":"provider-sandbox","envdAccessToken":"token","trafficAccessToken":"traffic-token","domain":"attacker.example"}"#,
    )]);

    let access = client
        .create_sandbox(ControlCreateSandbox {
            template_id: "template".to_owned(),
            metadata: SandboxMetadata::from([("sandbox_agent_id".to_owned(), "agent".to_owned())]),
            allow_public_egress: false,
            denied_destinations: vec!["203.0.113.0/24".to_owned()],
            idle_timeout_seconds: 600,
        })
        .await
        .expect("create should decode");

    assert_eq!(access.sandbox_id, "provider-sandbox");
    assert_eq!(access.domain, "e2b.app");
    assert_eq!(access.traffic_access_token, "traffic-token");
    let body: Value = serde_json::from_slice(
        only_request(&requests)
            .body
            .as_deref()
            .expect("create body"),
    )
    .expect("body should be JSON");
    assert_eq!(body["secure"], true);
    assert_eq!(body["allow_internet_access"], false);
    assert_eq!(body["autoPause"], true);
    assert_eq!(body["autoPauseMemory"], true);
    assert_eq!(body["autoResume"]["enabled"], false);
    assert_eq!(body["network"]["allowPublicTraffic"], false);
    let denies = body["network"]["denyOut"].as_array().expect("deny list");
    assert!(denies.iter().any(|value| value == "127.0.0.0/8"));
    assert!(denies.iter().any(|value| value == "203.0.113.0/24"));
    assert!(!denies.iter().any(|value| value == "0.0.0.0/8"));
    assert!(!denies.iter().any(|value| value == "::/128"));
    assert!(!String::from_utf8_lossy(&only_request(&requests).body.unwrap()).contains("token"));
}

#[tokio::test]
async fn snapshots_are_filtered_by_opaque_name_and_exact_id() {
    let response = r#"[
        {"snapshotID":"team/match:one","names":["team/sandbox-operation:one"]},
        {"snapshotID":"team/other:one","names":["team/out-of-band:one"]}
    ]"#;
    let (client, requests) = recording_client(vec![
        json_response(200, response),
        json_response(200, response),
    ]);

    let filtered = client
        .list_snapshots("source", "sandbox-operation")
        .await
        .expect("filtered list");
    let exact = client
        .get_snapshot("source", "out-of-band", "team/other:one")
        .await
        .expect("exact snapshot");

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].snapshot_id, "team/match:one");
    assert_eq!(exact.snapshot_id, "team/other:one");
    let requests = requests.lock().expect("request lock");
    assert!(requests[0].path_and_query.contains("sandboxID=source"));
    assert!(requests[1].path_and_query.contains("sandboxID=source"));
    assert!(!requests[0].path_and_query.contains("name="));
}

#[tokio::test]
async fn statuses_and_missing_secure_token_are_typed() {
    let (missing, _) = recording_client(vec![json_response(404, "{}")]);
    let (rate_limited, _) = recording_client(vec![json_response(429, "{}")]);
    let (ambiguous, _) = recording_client(vec![json_response(500, "{}")]);
    let (tokenless, _) = recording_client(vec![json_response(
        201,
        r#"{"sandboxID":"provider-sandbox"}"#,
    )]);
    let (traffic_tokenless, _) = recording_client(vec![json_response(
        201,
        r#"{"sandboxID":"provider-sandbox","envdAccessToken":"token"}"#,
    )]);

    assert!(matches!(
        missing.get_sandbox("missing").await,
        Err(E2bAdapterError::NotFound)
    ));
    assert!(matches!(
        rate_limited.get_sandbox("limited").await,
        Err(E2bAdapterError::Unavailable)
    ));
    assert!(matches!(
        ambiguous.create_snapshot("source", "snapshot").await,
        Err(E2bAdapterError::DeliveryAmbiguous)
    ));
    assert!(matches!(
        tokenless.create_sandbox(create_request()).await,
        Err(E2bAdapterError::DeliveryAmbiguous)
    ));
    assert!(matches!(
        traffic_tokenless.create_sandbox(create_request()).await,
        Err(E2bAdapterError::DeliveryAmbiguous)
    ));
}

#[tokio::test]
async fn accepted_snapshot_with_malformed_body_preserves_delivery_ambiguity() {
    let (client, _) = recording_client(vec![json_response(201, "not-json")]);

    let result = client.create_snapshot("source", "snapshot").await;

    assert!(matches!(result, Err(E2bAdapterError::DeliveryAmbiguous)));
}

#[tokio::test]
async fn accepted_safe_read_with_malformed_body_is_unavailable() {
    let (client, _) = recording_client(vec![json_response(200, "not-json")]);

    let result = client.get_sandbox("sandbox").await;

    assert!(matches!(result, Err(E2bAdapterError::Unavailable)));
}

#[tokio::test]
async fn existing_sandbox_calls_reject_mismatched_response_identity() {
    let (get_client, _) = recording_client(vec![json_response(
        200,
        r#"{"sandboxID":"different","state":"running"}"#,
    )]);
    let (connect_client, _) = recording_client(vec![json_response(
        200,
        r#"{"sandboxID":"different","envdAccessToken":"token","trafficAccessToken":"traffic-token"}"#,
    )]);

    assert!(matches!(
        get_client.get_sandbox("expected").await,
        Err(E2bAdapterError::Internal(_))
    ));
    assert!(matches!(
        connect_client.connect_sandbox("expected").await,
        Err(E2bAdapterError::Internal(_))
    ));
}

#[test]
fn adapter_debug_output_redacts_all_secrets() {
    let config = E2bAdapterConfig::new(
        "e2b",
        "https://api.e2b.app",
        "api-key-secret",
        HashMap::from([(
            "general".to_owned(),
            E2bProfile {
                template: "base".to_owned(),
                allow_public_egress: true,
                denied_destinations: vec!["203.0.113.10/32".to_owned()],
            },
        )]),
        600,
    )
    .expect("valid config");
    let access = ControlSandboxAccess {
        sandbox_id: "sandbox-secret".to_owned(),
        domain: "e2b.app".to_owned(),
        envd_access_token: "token-secret".to_owned(),
        traffic_access_token: "traffic-token-secret".to_owned(),
    };

    let debug = format!("{config:?} {access:?}");
    assert!(!debug.contains("api-key-secret"));
    assert!(!debug.contains("sandbox-secret"));
    assert!(!debug.contains("token-secret"));
    assert!(!debug.contains("traffic-token-secret"));
}

fn create_request() -> ControlCreateSandbox {
    ControlCreateSandbox {
        template_id: "base".to_owned(),
        metadata: SandboxMetadata::new(),
        allow_public_egress: false,
        denied_destinations: Vec::new(),
        idle_timeout_seconds: 600,
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
        ReqwestE2bControlApi::with_transport(transport, "e2b.app", 600),
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
