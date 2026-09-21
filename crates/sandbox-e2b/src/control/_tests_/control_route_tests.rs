//! Authenticated control-route construction regressions.

use std::sync::Arc;

use unimock::Unimock;

use crate::{E2bAdapterError, E2bControlApi};

use super::ReqwestE2bControlApi;

#[tokio::test]
async fn dot_provider_identifiers_fail_before_transport() {
    let client = ReqwestE2bControlApi {
        transport: Arc::new(Unimock::new(())),
        sandbox_domain: "e2b.app".to_owned(),
        idle_timeout_seconds: 600,
    };

    for provider_id in [".", ".."] {
        assert!(matches!(
            client.pause_sandbox(provider_id).await,
            Err(E2bAdapterError::InvalidRequest)
        ));
    }
}
