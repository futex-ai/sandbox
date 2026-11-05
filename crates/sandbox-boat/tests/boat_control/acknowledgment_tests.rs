//! Resource identity and completion acknowledgments fail closed.

use sandbox_boat::{BoatAdapterError, control::api::BoatControlApi};
use sandbox_interface::{Error, ProviderRef, SandboxLifecycle};
use serde_json::json;

use super::support::{fixture::Fixture, requests, state::Override, wire::Reply};

#[tokio::test]
async fn accepted_creates_and_existing_reads_validate_returned_resource_identity() {
    let fixture = Fixture::start().await;
    fixture.state.lock().unwrap().create_reply = Some(Reply::json(
        202,
        json!({"ok":true,"type":"sandbox.created","status":"ready","ttlSeconds":21600,
        "sandbox":{"id":"..","state":"ready"}}),
    ));
    let request = requests::create();
    assert!(matches!(
        fixture.backend.create_sandbox(request.clone()).await,
        Err(Error::BackendUnavailable { .. })
    ));
    let recovered = fixture
        .backend
        .recover_sandbox_create(request)
        .await
        .unwrap()
        .unwrap();
    fixture.state.lock().unwrap().overrides.push_back(Override {
        method: "GET".to_owned(),
        path: format!("/api/v1/sandboxes/{}", recovered.provider_ref.as_str()),
        reply: Reply::json(
            200,
            json!({"ok":true,"type":"sandbox.info","sandbox":{"id":"bx_33333333","state":"ready"}}),
        ),
    });
    assert!(matches!(
        fixture
            .backend
            .inspect_sandbox(recovered.provider_ref)
            .await,
        Err(Error::BackendUnavailable { .. })
    ));
    fixture.state.lock().unwrap().overrides.push_back(Override { method:"GET".to_owned(),path:"/api/v1/saved-snapshots/exact-name".to_owned(),
        reply:Reply::json(200,json!({"ok":true,"type":"snapshot.saved.info","snapshot":{"name":"other-name","status":"ready","sourceSandboxId":"bx_abcdefgh"}})) });
    assert!(matches!(
        fixture
            .control(Default::default())
            .get_snapshot("exact-name")
            .await,
        Err(BoatAdapterError::InvalidResponse)
    ));
}

#[tokio::test]
async fn wrong_deletion_target_status_or_saved_ack_never_confirms_completion() {
    let fixture = Fixture::start().await;
    for (target, status, kind) in [
        ("bx_33333333", "completed", "sandbox"),
        ("bx_22222222", "future", "sandbox"),
        ("bx_22222222", "completed", "snapshot"),
    ] {
        fixture.state.lock().unwrap().overrides.push_back(Override { method:"DELETE".to_owned(),path:"/api/v1/sandboxes/bx_22222222".to_owned(),
            reply:Reply::json(202,json!({"ok":true,"type":"deletion.operation","operation":{"id":"bdop_0123456789abcdef0123456789abcdef","targetId":target,"kind":kind,"status":status}})) });
        assert!(matches!(
            fixture
                .backend
                .destroy_sandbox(ProviderRef::new("bx_22222222"))
                .await,
            Err(Error::BackendUnavailable { .. })
        ));
    }
    fixture.state.lock().unwrap().overrides.push_back(Override { method:"DELETE".to_owned(),path:"/api/v1/saved-snapshots/exact-name".to_owned(),
        reply:Reply::json(200,json!({"ok":true,"type":"snapshot.saved.deleted","status":"deleting","name":"exact-name"})) });
    assert!(matches!(
        fixture
            .control(Default::default())
            .delete_snapshot("exact-name")
            .await,
        Err(BoatAdapterError::InvalidResponse)
    ));
}
