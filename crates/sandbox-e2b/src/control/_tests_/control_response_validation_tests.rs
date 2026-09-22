//! Provider access and snapshot response validation regressions.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use sandbox_interface::SandboxLifetime;
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlCreateSandbox, E2bAdapterError, E2bControlApi, SandboxMetadata,
    control::http::{E2bHttpTransport, HttpResponse, send},
};

use super::ReqwestE2bControlApi;

#[tokio::test]
async fn accepted_create_rejects_blank_credentials_as_ambiguous() {
    let client = recording_client([
        r#"{"sandboxID":"sandbox","envdAccessToken":" ","trafficAccessToken":"traffic"}"#,
        r#"{"sandboxID":"sandbox","envdAccessToken":"envd","trafficAccessToken":"\t"}"#,
    ]);

    for _ in 0..2 {
        assert!(matches!(
            client.create_sandbox(create_request()).await,
            Err(E2bAdapterError::DeliveryAmbiguous)
        ));
    }
}

#[tokio::test]
async fn accepted_sandbox_create_rejects_unroutable_identity_as_ambiguous() {
    let client = recording_client([
        r#"{"sandboxID":"","envdAccessToken":"envd","trafficAccessToken":"traffic"}"#,
        r#"{"sandboxID":".","envdAccessToken":"envd","trafficAccessToken":"traffic"}"#,
        r#"{"sandboxID":"..","envdAccessToken":"envd","trafficAccessToken":"traffic"}"#,
        r#"{"sandboxID":"provider/path","envdAccessToken":"envd","trafficAccessToken":"traffic"}"#,
    ]);

    for _ in 0..4 {
        assert!(matches!(
            client.create_sandbox(create_request()).await,
            Err(E2bAdapterError::DeliveryAmbiguous)
        ));
    }
}

#[tokio::test]
async fn connect_rejects_missing_or_blank_credentials_as_unavailable() {
    let client = recording_client_with_responses([
        (200, r#"{"sandboxID":"sandbox","state":"running"}"#),
        (
            201,
            r#"{"sandboxID":"sandbox","trafficAccessToken":"traffic"}"#,
        ),
        (200, r#"{"sandboxID":"sandbox","state":"running"}"#),
        (
            201,
            r#"{"sandboxID":"sandbox","envdAccessToken":" ","trafficAccessToken":"traffic"}"#,
        ),
        (200, r#"{"sandboxID":"sandbox","state":"running"}"#),
        (201, r#"{"sandboxID":"sandbox","envdAccessToken":"envd"}"#),
        (200, r#"{"sandboxID":"sandbox","state":"running"}"#),
        (
            201,
            r#"{"sandboxID":"sandbox","envdAccessToken":"envd","trafficAccessToken":"\t"}"#,
        ),
    ]);

    for _ in 0..4 {
        assert!(matches!(
            client.connect_sandbox("sandbox").await,
            Err(E2bAdapterError::Unavailable)
        ));
    }
}

#[tokio::test]
async fn sandbox_inventory_rejects_unroutable_identity_as_unavailable() {
    let client = recording_client_with_status(
        200,
        [
            r#"[{"sandboxID":"","state":"running"}]"#,
            r#"[{"sandboxID":".","state":"running"}]"#,
            r#"[{"sandboxID":"..","state":"running"}]"#,
            r#"[{"sandboxID":"provider/path","state":"running"}]"#,
        ],
    );

    for _ in 0..4 {
        assert!(matches!(
            client.list_sandboxes(SandboxMetadata::new()).await,
            Err(E2bAdapterError::Unavailable)
        ));
    }
}

#[tokio::test]
async fn accepted_snapshot_create_rejects_unusable_identity_as_ambiguous() {
    let client = recording_client([
        r#"{"snapshotID":""}"#,
        r#"{"snapshotID":"."}"#,
        r#"{"snapshotID":".."}"#,
    ]);

    for _ in 0..3 {
        assert!(matches!(
            client.create_snapshot("sandbox", "operation").await,
            Err(E2bAdapterError::DeliveryAmbiguous)
        ));
    }
}

fn create_request() -> ControlCreateSandbox {
    ControlCreateSandbox {
        template_id: "base".to_owned(),
        metadata: SandboxMetadata::new(),
        allow_public_egress: false,
        denied_destinations: Vec::new(),
        idle_timeout_seconds: 600,
        lifetime: SandboxLifetime::IdleAutoPause,
    }
}

fn recording_client<const N: usize>(bodies: [&str; N]) -> ReqwestE2bControlApi {
    recording_client_with_status(201, bodies)
}

fn recording_client_with_status<const N: usize>(
    status: u16,
    bodies: [&str; N],
) -> ReqwestE2bControlApi {
    let responses = Arc::new(Mutex::new(VecDeque::from(bodies.map(|body| {
        HttpResponse {
            status,
            body: body.as_bytes().to_vec(),
            next_token: None,
        }
    }))));
    let transport: Arc<dyn E2bHttpTransport> =
        Arc::new(Unimock::new(send.each_call(matching!(_)).answers_arc({
            let responses = responses.clone();
            Arc::new(move |_, _| {
                Ok(responses
                    .lock()
                    .expect("response lock")
                    .pop_front()
                    .expect("unexpected control request"))
            })
        })));
    ReqwestE2bControlApi {
        transport,
        sandbox_domain: "e2b.app".to_owned(),
        idle_timeout_seconds: 600,
        lifetime_metadata_key: "sandbox_lifetime".to_owned(),
    }
}

fn recording_client_with_responses<const N: usize>(
    responses: [(u16, &str); N],
) -> ReqwestE2bControlApi {
    let responses = Arc::new(Mutex::new(VecDeque::from(responses.map(
        |(status, body)| HttpResponse {
            status,
            body: body.as_bytes().to_vec(),
            next_token: None,
        },
    ))));
    let transport: Arc<dyn E2bHttpTransport> =
        Arc::new(Unimock::new(send.each_call(matching!(_)).answers_arc({
            let responses = responses.clone();
            Arc::new(move |_, _| {
                Ok(responses
                    .lock()
                    .expect("response lock")
                    .pop_front()
                    .expect("unexpected control request"))
            })
        })));
    ReqwestE2bControlApi {
        transport,
        sandbox_domain: "e2b.app".to_owned(),
        idle_timeout_seconds: 600,
        lifetime_metadata_key: "sandbox_lifetime".to_owned(),
    }
}
