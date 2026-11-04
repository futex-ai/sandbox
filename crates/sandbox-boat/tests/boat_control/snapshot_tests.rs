//! Saved captures preserve exact source identity and disposable restore settings.

use sandbox_boat::{
    BoatAdapterError,
    control::{api::BoatControlApi, types::BoatSaveSnapshot},
};
use sandbox_interface::{
    BackendInspectSnapshotRequest, BackendSnapshot, BackendSnapshotCreateOutcome,
    BackendSnapshotRecovery, Error, ProviderRef, SandboxLifecycle, SandboxSnapshots, SnapshotState,
};
use serde_json::json;

use super::support::{
    fixture::Fixture,
    requests,
    state::{self, CAPTURE_SOURCE},
};

#[tokio::test]
async fn saving_is_pending_ready_is_recovered_and_restore_keeps_disposable_settings() {
    let fixture = Fixture::start().await;
    fixture.state.lock().unwrap().snapshot_status = "saving".to_owned();
    let request = requests::snapshot("owned-capture");
    assert_eq!(
        fixture
            .backend
            .snapshot_inventory(
                request.source_provider_ref.clone(),
                request.correlation_name.clone()
            )
            .await
            .unwrap()
            .snapshots,
        []
    );
    assert_eq!(
        fixture
            .backend
            .create_snapshot(request.clone())
            .await
            .unwrap(),
        BackendSnapshotCreateOutcome::InProgress
    );
    assert_eq!(
        fixture
            .backend
            .recover_snapshot_create(request.clone())
            .await
            .unwrap(),
        BackendSnapshotRecovery::InProgress
    );
    fixture
        .state
        .lock()
        .unwrap()
        .snapshots
        .get_mut("owned-capture")
        .unwrap()["status"] = "ready".into();
    let recovered = fixture
        .backend
        .recover_snapshot_create(request.clone())
        .await
        .unwrap();
    let BackendSnapshotRecovery::Recovered(snapshot) = recovered else {
        panic!("expected recovered snapshot, got {recovered:?}");
    };
    assert_eq!(snapshot.state, SnapshotState::Ready);
    assert_eq!(snapshot.provider_ref.as_str(), "owned-capture");
    assert_eq!(
        fixture
            .backend
            .inspect_snapshot(BackendInspectSnapshotRequest {
                provider_ref: snapshot.provider_ref.clone(),
                source_provider_ref: request.source_provider_ref,
                correlation_name: request.correlation_name,
            })
            .await
            .unwrap(),
        snapshot
    );
    let mut restore = requests::create();
    restore.snapshot_provider_ref = Some(snapshot.provider_ref.clone());
    let restored = fixture
        .backend
        .create_sandbox(restore.clone())
        .await
        .unwrap();
    assert_eq!(
        fixture
            .backend
            .recover_sandbox_create(restore)
            .await
            .unwrap()
            .unwrap(),
        restored
    );
    let calls = fixture.captured();
    let payload = &calls
        .iter()
        .find(|call| call.method == "POST" && call.path == "/api/v1/sandboxes")
        .unwrap()
        .body;
    assert_eq!(payload["from"], "owned-capture");
    assert_eq!(payload["noEnv"], true);
    assert_eq!(payload["snapshots"], false);
    assert_eq!(payload["monitoring"], false);
    fixture
        .backend
        .delete_snapshot(snapshot.provider_ref.clone())
        .await
        .unwrap();
    fixture
        .backend
        .delete_snapshot(snapshot.provider_ref)
        .await
        .unwrap();
    let deletes = fixture
        .captured()
        .into_iter()
        .filter(|call| call.method == "DELETE")
        .collect::<Vec<_>>();
    assert_eq!(deletes.len(), 2);
    assert!(
        deletes
            .iter()
            .all(|call| !call.headers.contains_key("x-ascii-confirm-delete"))
    );
    assert!(
        fixture
            .state
            .lock()
            .unwrap()
            .sandboxes
            .contains_key(CAPTURE_SOURCE)
    );
}

#[tokio::test]
async fn ambiguous_saved_capture_recovers_by_exact_name_without_another_post() {
    let fixture = Fixture::start().await;
    fixture.state.lock().unwrap().save_reply = Some(state::error(503, "upstream_error"));
    let request = requests::snapshot("ambiguous-capture");
    assert_eq!(
        fixture
            .backend
            .create_snapshot(request.clone())
            .await
            .unwrap(),
        BackendSnapshotCreateOutcome::DeliveryAmbiguous
    );
    assert_eq!(
        fixture
            .backend
            .recover_snapshot_create(request)
            .await
            .unwrap(),
        BackendSnapshotRecovery::Recovered(BackendSnapshot {
            provider_ref: ProviderRef::new("ambiguous-capture"),
            state: SnapshotState::Ready
        })
    );
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
async fn disposable_sources_cannot_be_captured_by_adapter_or_provider_fixture() {
    let fixture = Fixture::start().await;
    let source = fixture
        .backend
        .create_sandbox(requests::create())
        .await
        .unwrap();
    let mut capture = requests::snapshot("forbidden-capture");
    capture.source_provider_ref = source.provider_ref.clone();
    assert!(matches!(
        fixture.backend.create_snapshot(capture).await,
        Err(Error::Internal(_))
    ));
    let control = fixture.control(Default::default());
    assert!(matches!(
        control
            .save_snapshot(BoatSaveSnapshot {
                sandbox_id: source.provider_ref.as_str().to_owned(),
                name: "provider-forbidden".to_owned(),
            })
            .await,
        Err(BoatAdapterError::InvalidRequest)
    ));
    assert_eq!(fixture.state.lock().unwrap().snapshots.len(), 0);
    assert_eq!(
        fixture
            .captured()
            .iter()
            .filter(|call| call.method == "POST" && call.path == "/api/v1/saved-snapshots")
            .count(),
        1
    );
}

#[tokio::test]
async fn existing_name_wrong_source_and_failed_save_preserve_identity_without_post() {
    let fixture = Fixture::start().await;
    fixture.state.lock().unwrap().snapshots.insert(
        "collision".to_owned(),
        json!({"name":"collision","status":"ready","sourceSandboxId":"bx_33333333"}),
    );
    let request = requests::snapshot("collision");
    assert!(matches!(
        fixture.backend.create_snapshot(request.clone()).await,
        Err(Error::SnapshotReconciliationRequired { .. })
    ));
    assert_eq!(
        fixture
            .backend
            .recover_snapshot_create(request.clone())
            .await
            .unwrap(),
        BackendSnapshotRecovery::ReconciliationRequired
    );
    fixture
        .state
        .lock()
        .unwrap()
        .snapshots
        .get_mut("collision")
        .unwrap()["sourceSandboxId"] = CAPTURE_SOURCE.into();
    fixture
        .state
        .lock()
        .unwrap()
        .snapshots
        .get_mut("collision")
        .unwrap()["status"] = "failed".into();
    assert_eq!(
        fixture
            .backend
            .recover_snapshot_create(request.clone())
            .await
            .unwrap(),
        BackendSnapshotRecovery::Recovered(BackendSnapshot {
            provider_ref: ProviderRef::new("collision"),
            state: SnapshotState::Failed,
        })
    );
    let inspected = fixture
        .backend
        .inspect_snapshot(BackendInspectSnapshotRequest {
            provider_ref: ProviderRef::new("collision"),
            source_provider_ref: request.source_provider_ref.clone(),
            correlation_name: "collision".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(inspected.state, SnapshotState::Failed);
    let mut before = request;
    before.before.snapshots = vec![ProviderRef::new("collision")];
    assert_eq!(
        fixture
            .backend
            .recover_snapshot_create(before)
            .await
            .unwrap(),
        BackendSnapshotRecovery::ReconciliationRequired
    );
    assert_eq!(
        fixture
            .captured()
            .iter()
            .filter(|call| call.method == "POST")
            .count(),
        0
    );
    assert_eq!(
        fixture.state.lock().unwrap().snapshots["collision"]["status"],
        "failed"
    );
}
