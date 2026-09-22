//! Authenticated control-route construction regressions.

use std::sync::Arc;

use unimock::Unimock;

use crate::{E2bAdapterError, E2bControlApi};

use super::ReqwestE2bControlApi;

#[test]
fn public_control_client_rejects_unsafe_configuration() {
    for (api_base, api_key, sandbox_domain, idle_timeout_seconds) in [
        ("http://api.e2b.app", "api-key", "e2b.app", 600),
        ("https://api.e2b.app/path", "api-key", "e2b.app", 600),
        ("https://api.e2b.app", "", "e2b.app", 600),
        ("https://api.e2b.app", "api-key", "https://e2b.app", 600),
        ("https://api.e2b.app", "api-key", "e2b.app", 0),
    ] {
        assert!(matches!(
            ReqwestE2bControlApi::new(api_base, api_key, sandbox_domain, idle_timeout_seconds,),
            Err(E2bAdapterError::InvalidRequest)
        ));
    }
}

#[tokio::test]
async fn unsafe_provider_identifiers_fail_before_transport() {
    let client = client_without_transport();

    for provider_id in ["", ".", ".."] {
        assert!(matches!(
            client.pause_sandbox(provider_id).await,
            Err(E2bAdapterError::InvalidRequest)
        ));
    }
}

#[tokio::test]
async fn empty_snapshot_scope_fails_before_transport() {
    for (sandbox_id, correlation_name) in [("", "operation"), ("sandbox", "")] {
        assert!(matches!(
            client_without_transport()
                .list_snapshots(sandbox_id, correlation_name)
                .await,
            Err(E2bAdapterError::InvalidRequest)
        ));
    }
}

#[tokio::test]
async fn empty_snapshot_creation_name_fails_before_transport() {
    assert!(matches!(
        client_without_transport()
            .create_snapshot("sandbox", "")
            .await,
        Err(E2bAdapterError::InvalidRequest)
    ));
}

fn client_without_transport() -> ReqwestE2bControlApi {
    ReqwestE2bControlApi {
        transport: Arc::new(Unimock::new(())),
        sandbox_domain: "e2b.app".to_owned(),
        idle_timeout_seconds: 600,
        lifetime_metadata_key: "sandbox_lifetime".to_owned(),
    }
}
