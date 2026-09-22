//! Call-specific sandbox connection timeout regressions.

use std::sync::{Arc, Mutex};

use serde_json::Value;
use unimock::{MockFn, Unimock, matching};

use crate::control::http::{E2bHttpTransport, HttpRequest, HttpResponse, send};
use crate::{E2bAdapterError, E2bControlApi};

use super::ReqwestE2bControlApi;

#[tokio::test]
async fn call_specific_connect_timeout_is_forwarded() {
    let request = Arc::new(Mutex::new(None));
    let transport: Arc<dyn E2bHttpTransport> = Arc::new(Unimock::new(
        send.next_call(matching!(_)).answers_arc({
            let request = request.clone();
            Arc::new(move |_, sent: HttpRequest| {
                *request.lock().expect("request lock") = Some(sent);
                Ok(HttpResponse {
                    status: 200,
                    body: br#"{"sandboxID":"sandbox","envdAccessToken":"envd","trafficAccessToken":"traffic"}"#.to_vec(),
                    next_token: None,
                })
            })
        }),
    ));
    let client = client(transport);

    client
        .connect_sandbox_with_timeout("sandbox", 3600)
        .await
        .expect("connect should decode");

    let body = request
        .lock()
        .expect("request lock")
        .as_ref()
        .and_then(|request| request.body.as_deref())
        .map(serde_json::from_slice::<Value>)
        .transpose()
        .expect("connect body should be JSON")
        .expect("connect body");
    assert_eq!(body["timeout"], 3600);
}

#[tokio::test]
async fn zero_call_specific_connect_timeout_fails_before_transport() {
    let result = client(Arc::new(Unimock::new(())))
        .connect_sandbox_with_timeout("sandbox", 0)
        .await;

    assert!(matches!(result, Err(E2bAdapterError::InvalidRequest)));
}

fn client(transport: Arc<dyn E2bHttpTransport>) -> ReqwestE2bControlApi {
    ReqwestE2bControlApi {
        transport,
        sandbox_domain: "e2b.app".to_owned(),
        idle_timeout_seconds: 600,
    }
}
