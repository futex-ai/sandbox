//! Image-snapshot ambiguous-delivery recovery tests.

use std::{collections::HashMap, sync::Arc, time::Duration};

use sandbox_interface::{OperationId, ProviderRef, SnapshotId};
use unimock::{MockFn, Unimock, matching};

use crate::{
    ControlSandboxAccess, ControlSnapshot, E2bAdapterConfig, E2bControlApiMock, E2bProfile,
    error::Error as AdapterError,
};

use super::{
    configured::E2bSandboxBackend,
    image_snapshot::{self, SnapshotRecoverySleeperMock},
};

#[tokio::test]
async fn image_snapshot_waits_for_ambiguous_delivery_to_become_visible() {
    let control = Unimock::new((
        E2bControlApiMock::list_snapshots
            .next_call(matching!("source", "sandbox-image"))
            .returns(Ok(Vec::new())),
        E2bControlApiMock::create_snapshot
            .next_call(matching!("source", "sandbox-image"))
            .returns(Err(AdapterError::DeliveryAmbiguous)),
        E2bControlApiMock::list_snapshots
            .next_call(matching!("source", "sandbox-image"))
            .returns(Ok(Vec::new())),
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("source"))
            .returns(Ok(access())),
        E2bControlApiMock::list_snapshots
            .next_call(matching!("source", "sandbox-image"))
            .returns(Ok(vec![ControlSnapshot {
                snapshot_id: "snapshot".to_owned(),
            }])),
        E2bControlApiMock::connect_sandbox
            .next_call(matching!("source"))
            .returns(Ok(access())),
    ));
    let sleeper = Unimock::new(
        SnapshotRecoverySleeperMock::sleep
            .next_call(matching!(_))
            .answers(&|_, duration| assert_eq!(duration, Duration::from_secs(1))),
    );
    let backend = E2bSandboxBackend {
        config: config(),
        control: Arc::new(control),
        processes: Arc::new(Unimock::new(())),
        snapshot_recovery_sleeper: Arc::new(sleeper),
    };

    let provider_ref = image_snapshot::realize(
        &backend,
        SnapshotId::new(),
        OperationId::new(),
        "sandbox-image".to_owned(),
        ProviderRef::new("source"),
    )
    .await
    .expect("snapshot should be adopted after it becomes visible");

    assert_eq!(provider_ref.as_str(), "snapshot");
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

fn access() -> ControlSandboxAccess {
    ControlSandboxAccess {
        sandbox_id: "source".to_owned(),
        domain: "e2b.app".to_owned(),
        envd_access_token: "call-local-token".to_owned(),
        traffic_access_token: "traffic-token".to_owned(),
    }
}
