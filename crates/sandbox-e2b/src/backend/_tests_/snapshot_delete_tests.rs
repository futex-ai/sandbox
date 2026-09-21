//! E2B snapshot deletion boundary tests.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{Error, ProviderRef, SandboxBackend};
use unimock::{MockFn, Unimock, matching};

use crate::{E2bAdapterConfig, E2bAdapterError, E2bControlApiMock, E2bProfile, E2bSandboxBackend};

#[tokio::test]
async fn provider_snapshot_conflict_stays_opaque_at_backend_boundary() {
    let control = Unimock::new(
        E2bControlApiMock::delete_snapshot
            .next_call(matching!("snapshot"))
            .returns(Err(E2bAdapterError::InvalidRequest)),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(Unimock::new(())));

    let error = backend
        .delete_snapshot(ProviderRef::new("snapshot"))
        .await
        .expect_err("provider conflict should remain a redacted backend error");

    assert!(matches!(error, Error::Internal(_)));
}

fn config() -> E2bAdapterConfig {
    E2bAdapterConfig::new(
        "configured-e2b",
        "https://api.e2b.app",
        "api-key",
        HashMap::from([(
            "general".to_owned(),
            E2bProfile {
                template: "base".to_owned(),
                allow_public_egress: false,
                denied_destinations: vec!["203.0.113.10/32".to_owned()],
            },
        )]),
        600,
    )
    .expect("valid adapter config")
}
