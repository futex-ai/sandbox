//! E2B snapshot deletion boundary tests.

use std::{collections::HashMap, sync::Arc};

use sandbox_interface::{BackendInspectSnapshotRequest, Error, ProviderRef, SandboxBackend};
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSnapshot, E2bAdapterConfig, E2bAdapterError, E2bControlApiMock, E2bProfile,
    E2bSandboxBackend,
};

#[tokio::test]
async fn snapshot_inspection_rejects_a_mismatched_provider_identity() {
    let control = Unimock::new(
        E2bControlApiMock::get_snapshot
            .next_call(matching!("source", "operation", "expected"))
            .returns(Ok(ControlSnapshot {
                snapshot_id: "different".to_owned(),
            })),
    );
    let backend =
        E2bSandboxBackend::with_transports(config(), Arc::new(control), Arc::new(Unimock::new(())));

    let error = backend
        .inspect_snapshot(BackendInspectSnapshotRequest {
            provider_ref: ProviderRef::new("expected"),
            source_provider_ref: ProviderRef::new("source"),
            correlation_name: "operation".to_owned(),
        })
        .await
        .expect_err("snapshot inspection must preserve the requested identity");

    assert!(matches!(error, Error::Internal(_)));
}

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
