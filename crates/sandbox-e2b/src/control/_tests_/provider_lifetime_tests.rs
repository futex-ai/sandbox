//! Public control creation must enforce E2B's own lifetime maximum.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use sandbox_interface::SandboxLifetime;
use serde_json::Value;
use unimock::{MockFn, Unimock, matching};

use crate::control::http::{HttpResponse, send};
use crate::{ControlCreateSandbox, E2bAdapterError, E2bControlApi, ReqwestE2bControlApi};

#[tokio::test]
async fn public_create_rejects_over_e2b_limit_before_transport() {
    let calls = Arc::new(AtomicUsize::new(0));
    let transport = Unimock::new(send.each_call(matching!(_)).answers_arc({
        let calls = calls.clone();
        Arc::new(move |_, request| {
            calls.fetch_add(1, Ordering::SeqCst);
            let body: Value = serde_json::from_slice(request.body.as_deref().unwrap()).unwrap();
            assert_eq!(body["timeout"], 3600);
            Ok(HttpResponse { status: 201, body: br#"{"sandboxID":"created","envdAccessToken":"envd","trafficAccessToken":"traffic"}"#.to_vec(), next_token: None })
        })
    }).at_least_times(0));
    let client = ReqwestE2bControlApi {
        transport: Arc::new(transport),
        sandbox_domain: "e2b.app".to_owned(),
        idle_timeout_seconds: 600,
        lifetime_metadata_key: "sandbox_lifetime".to_owned(),
    };

    let accepted = client
        .create_sandbox(ControlCreateSandbox {
            template_id: "base".to_owned(),
            metadata: Default::default(),
            allow_public_egress: true,
            denied_destinations: Vec::new(),
            allowed_destinations: None,
            idle_timeout_seconds: 600,
            lifetime: SandboxLifetime::OneShot {
                max_lifetime: Duration::from_secs(3600),
            },
        })
        .await
        .unwrap();
    assert_eq!(accepted.sandbox_id, "created");
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    for seconds in [3601, 21_600] {
        let result = client
            .create_sandbox(ControlCreateSandbox {
                template_id: "base".to_owned(),
                metadata: Default::default(),
                allow_public_egress: true,
                denied_destinations: Vec::new(),
                allowed_destinations: None,
                idle_timeout_seconds: 600,
                lifetime: SandboxLifetime::OneShot {
                    max_lifetime: Duration::from_secs(seconds),
                },
            })
            .await;

        assert!(
            matches!(result, Err(E2bAdapterError::InvalidRequest)),
            "expected E2B lifetime refusal, received {result:?}"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
