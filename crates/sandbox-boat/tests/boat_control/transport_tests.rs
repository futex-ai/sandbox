//! Authenticated routes, HTTPS trust, redirect isolation, and response budgets.

use std::{sync::Arc, time::Duration};

use sandbox_boat::{
    BoatAdapterConfig, BoatAdapterError, BoatProfile,
    control::{api::BoatControlApi, client::ReqwestBoatControlApi},
    transport::{
        http::{BoatHttpTransport, ReqwestBoatHttpTransport},
        request::{BoatHttpMethod, BoatHttpRequest, BoatRoute},
        settings::BoatHttpSettings,
    },
};
use sandbox_interface::SandboxLifecycle;
use serde_json::json;

use super::support::{
    fixture::{self, Fixture},
    requests,
    state::Override,
    wire::Reply,
};

#[test]
fn configuration_rejects_unsafe_origins_keys_and_unenforceable_network_requirements() {
    let profile = BoatProfile {
        provider_size: "default".to_owned(),
        allow_public_egress: true,
        denied_destinations: Vec::new(),
    };
    for origin in [
        "http://boat.dev",
        "https://user:secret@boat.dev",
        "https://boat.dev/other",
        "https://boat.dev/api/v1?key=x",
        "https://boat.dev/#fragment",
    ] {
        assert!(
            matches!(
                BoatAdapterConfig::new(
                    "boat",
                    origin,
                    "key",
                    [("general".to_owned(), profile.clone())].into()
                ),
                Err(BoatAdapterError::InvalidRequest)
            ),
            "{origin}"
        );
    }
    for key in ["", " key", "key ", "key\r\nother"] {
        assert!(matches!(
            BoatAdapterConfig::new(
                "boat",
                "https://boat.dev",
                key,
                [("general".to_owned(), profile.clone())].into()
            ),
            Err(BoatAdapterError::InvalidRequest)
        ));
    }
    for restricted in [
        BoatProfile {
            allow_public_egress: false,
            ..profile.clone()
        },
        BoatProfile {
            denied_destinations: vec!["10.0.0.0/8".to_owned()],
            ..profile.clone()
        },
    ] {
        assert!(matches!(
            BoatAdapterConfig::new(
                "boat",
                "https://boat.dev",
                "key",
                [("general".to_owned(), restricted)].into()
            ),
            Err(BoatAdapterError::InvalidRequest)
        ));
    }
    let config = BoatAdapterConfig::new(
        "boat",
        "https://boat.dev/api/v1",
        "secret-key",
        [("general".to_owned(), profile)].into(),
    )
    .unwrap();
    assert_eq!(config.api_base(), "https://boat.dev/api/v1");
    assert!(!format!("{config:?}").contains("secret-key"));
}

#[tokio::test]
async fn unsafe_route_segments_are_rejected_before_any_credentialed_request() {
    let fixture = Fixture::start().await;
    let control = fixture.control(Default::default());
    for id in [
        "",
        ".",
        "..",
        "bx_22222222/../limits",
        "BX_22222222",
        "bx_00000000",
        "bx_22222222?x=y",
    ] {
        assert!(
            matches!(
                control.get_sandbox(id).await,
                Err(BoatAdapterError::InvalidRequest)
            ),
            "{id}"
        );
        assert!(
            matches!(
                control.delete_sandbox(id).await,
                Err(BoatAdapterError::InvalidRequest)
            ),
            "{id}"
        );
    }
    for name in ["", ".", "..", "a/b", "%2e%2e", "A-name", "with?query"] {
        assert!(
            matches!(
                control.get_snapshot(name).await,
                Err(BoatAdapterError::InvalidRequest)
            ),
            "{name}"
        );
        assert!(
            matches!(
                control.delete_snapshot(name).await,
                Err(BoatAdapterError::InvalidRequest)
            ),
            "{name}"
        );
    }
    assert_eq!(fixture.captured().len(), 0);
}

#[tokio::test]
async fn verified_tls_and_redirect_policy_keep_credentials_at_the_configured_origin() {
    let fixture = Fixture::start().await;
    let untrusted = ReqwestBoatControlApi::with_transport(Arc::new(
        ReqwestBoatHttpTransport::new(fixture::config(&fixture.origin)).unwrap(),
    ));
    assert!(matches!(
        untrusted.limits().await,
        Err(BoatAdapterError::Unavailable)
    ));
    assert_eq!(fixture.captured().len(), 0);
    let target = Fixture::start().await;
    let mut roots = fixture.ca.clone();
    roots.extend_from_slice(&target.ca);
    let transport = ReqwestBoatHttpTransport::with_settings(
        fixture::config(&fixture.origin),
        Default::default(),
        Some(&roots),
    )
    .unwrap();
    let control = ReqwestBoatControlApi::with_transport(Arc::new(transport));
    let mut reply = Reply::json(307, json!({}));
    reply.headers.push((
        "Location".to_owned(),
        format!("{}/api/v1/limits", target.origin),
    ));
    fixture.state.lock().unwrap().overrides.push_back(Override {
        method: "GET".to_owned(),
        path: "/api/v1/limits".to_owned(),
        reply,
    });
    assert!(matches!(
        control.limits().await,
        Err(BoatAdapterError::Unavailable)
    ));
    assert_eq!(fixture.captured().len(), 1);
    assert_eq!(target.captured().len(), 0);
}

#[tokio::test]
async fn response_bytes_and_body_deadlines_fail_without_replaying_mutations() {
    let fixture = Fixture::start().await;
    let settings = BoatHttpSettings {
        max_response_bytes: 32,
        ..Default::default()
    };
    assert!(matches!(
        fixture.control(settings).limits().await,
        Err(BoatAdapterError::ResponseTooLarge)
    ));
    let mut chunked = Reply::json(200, json!({"padding":"x".repeat(100)}));
    chunked
        .headers
        .push(("Transfer-Encoding".to_owned(), "chunked".to_owned()));
    fixture.state.lock().unwrap().overrides.push_back(Override {
        method: "GET".to_owned(),
        path: "/api/v1/limits".to_owned(),
        reply: chunked,
    });
    assert!(matches!(
        fixture.control(settings).limits().await,
        Err(BoatAdapterError::ResponseTooLarge)
    ));
    let settings = BoatHttpSettings {
        connect_timeout: Duration::from_secs(1),
        read_timeout: Duration::from_millis(50),
        request_timeout: Duration::from_secs(1),
        ..Default::default()
    };
    let mut reply = Reply::json(200, json!({"ok":true}));
    reply.stall = true;
    fixture.state.lock().unwrap().overrides.push_back(Override {
        method: "GET".to_owned(),
        path: "/api/v1/limits".to_owned(),
        reply,
    });
    assert!(matches!(
        fixture.control(settings).limits().await,
        Err(BoatAdapterError::Unavailable)
    ));
    fixture.state.lock().unwrap().create_reply = Some(Reply {
        status: 202,
        body: vec![b'x'; 1024 * 1024 + 1],
        headers: Vec::new(),
        stall: false,
        segments: Vec::new(),
    });
    assert!(matches!(
        fixture.backend.create_sandbox(requests::create()).await,
        Err(sandbox_interface::Error::BackendUnavailable { .. })
    ));
    assert_eq!(
        fixture
            .captured()
            .iter()
            .filter(|call| call.method == "POST")
            .count(),
        1
    );
}

#[tokio::test]
async fn deletion_operations_are_forbidden_by_fixture_and_excluded_from_typed_routes() {
    let fixture = Fixture::start().await;
    let client = reqwest::Client::builder()
        .add_root_certificate(reqwest::Certificate::from_pem(&fixture.ca).unwrap())
        .build()
        .unwrap();
    let response = client
        .get(format!(
            "{}/api/v1/deletion-operations/bdop_0123456789abcdef0123456789abcdef",
            fixture.origin
        ))
        .bearer_auth("fixture-key")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 403);
    let transport = ReqwestBoatHttpTransport::with_settings(
        fixture::config(&fixture.origin),
        Default::default(),
        Some(&fixture.ca),
    )
    .unwrap();
    assert!(matches!(
        transport
            .send(BoatHttpRequest {
                method: BoatHttpMethod::Post,
                route: BoatRoute::Limits,
                body: None,
                idempotency_key: None
            })
            .await,
        Err(BoatAdapterError::InvalidRequest)
    ));
    assert_eq!(fixture.captured().len(), 1);
}
