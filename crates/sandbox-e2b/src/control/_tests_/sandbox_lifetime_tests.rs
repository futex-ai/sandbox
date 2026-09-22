//! E2B create-body lifetime encoding regressions.

use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

use sandbox_interface::SandboxLifetime;
use serde_json::Value;
use unimock::{MockFn, Unimock, matching};

use crate::{ControlCreateSandbox, E2bControlApi};

use super::ReqwestE2bControlApi;
use crate::control::http::{E2bHttpTransport, HttpRequest, HttpResponse, send};

#[tokio::test]
async fn idle_auto_pause_create_preserves_the_existing_exact_lifecycle_body() {
    let (client, requests) = recording_client();

    let access = client
        .create_sandbox(ControlCreateSandbox {
            template_id: "template".to_owned(),
            metadata: BTreeMap::from([("sandbox_agent_id".to_owned(), "agent".to_owned())]),
            allow_public_egress: false,
            denied_destinations: vec!["203.0.113.0/24".to_owned()],
            idle_timeout_seconds: 600,
            lifetime: SandboxLifetime::IdleAutoPause,
        })
        .await
        .expect("idle auto-pause create should decode");

    assert_eq!(access.domain, "e2b.app");
    assert_eq!(access.traffic_access_token.as_deref(), Some("traffic"));
    let request = requests.lock().expect("request lock")[0].clone();
    let body: Value = serde_json::from_slice(request.body.as_deref().expect("create body"))
        .expect("body should be JSON");
    assert_eq!(body.as_object().map(|body| body.len()), Some(9));
    assert_eq!(body["templateID"], "template");
    assert_eq!(body["metadata"]["sandbox_agent_id"], "agent");
    assert_eq!(body["secure"], true);
    assert_eq!(body["allow_internet_access"], false);
    assert_eq!(body["autoPause"], true);
    assert_eq!(body["autoPauseMemory"], true);
    assert_eq!(body["autoResume"]["enabled"], false);
    assert_eq!(body["timeout"], 600);
    assert_eq!(body["network"]["allowPublicTraffic"], false);
    assert_eq!(
        body["network"]["denyOut"],
        serde_json::json!([
            "10.0.0.0/8",
            "100.64.0.0/10",
            "127.0.0.0/8",
            "169.254.0.0/16",
            "172.16.0.0/12",
            "192.168.0.0/16",
            "203.0.113.0/24",
            "224.0.0.0/4",
            "::1/128",
            "fc00::/7",
            "fe80::/10"
        ])
    );
    assert!(
        !String::from_utf8_lossy(request.body.as_deref().expect("create body")).contains("token")
    );
}

#[tokio::test]
async fn one_shot_create_disables_pause_and_uses_its_maximum_lifetime() {
    let (client, requests) = recording_client();

    client
        .create_sandbox(ControlCreateSandbox {
            template_id: "base".to_owned(),
            metadata: BTreeMap::new(),
            allow_public_egress: false,
            denied_destinations: Vec::new(),
            idle_timeout_seconds: 600,
            lifetime: SandboxLifetime::OneShot {
                max_lifetime: Duration::from_secs(90),
            },
        })
        .await
        .expect("one-shot create should decode");

    let request = requests.lock().expect("request lock")[0].clone();
    let body: Value = serde_json::from_slice(request.body.as_deref().expect("create body"))
        .expect("body should be JSON");
    assert_eq!(body.as_object().map(|body| body.len()), Some(9));
    assert_eq!(body["autoPause"], false);
    assert_eq!(body["autoPauseMemory"], false);
    assert_eq!(body["autoResume"]["enabled"], false);
    assert_eq!(body["timeout"], 90);
}

fn recording_client() -> (ReqwestE2bControlApi, Arc<Mutex<Vec<HttpRequest>>>) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let responses = Arc::new(Mutex::new(VecDeque::from([HttpResponse {
        status: 201,
        body: br#"{"sandboxID":"provider-sandbox","envdAccessToken":"envd","trafficAccessToken":"traffic"}"#.to_vec(),
        next_token: None,
    }])));
    let transport: Arc<dyn E2bHttpTransport> =
        Arc::new(Unimock::new(send.each_call(matching!(_)).answers_arc({
            let requests = requests.clone();
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
