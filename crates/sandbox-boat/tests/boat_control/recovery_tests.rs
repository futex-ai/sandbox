//! Exact request recovery, eventual visibility, and optional legacy inventory.

use sandbox_interface::{Error, ProviderRef, SandboxLifecycle, SandboxLifetime, SandboxState};
use serde_json::json;

use super::support::{
    fixture::Fixture,
    requests,
    state::{self, Override},
    wire::Reply,
};

#[tokio::test]
async fn ambiguous_dispatch_recovers_with_reads_and_never_replays_post() {
    let fixture = Fixture::start().await;
    fixture.state.lock().unwrap().create_reply = Some(state::error(503, "upstream_error"));
    let request = requests::create();
    assert!(matches!(
        fixture.backend.create_sandbox(request.clone()).await,
        Err(Error::BackendUnavailable { .. })
    ));
    let recovered = fixture
        .backend
        .recover_sandbox_create(request.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.state, SandboxState::Ready);
    assert_eq!(
        fixture
            .backend
            .recover_sandbox_create(request)
            .await
            .unwrap()
            .unwrap(),
        recovered
    );
    let missing = fixture
        .backend
        .recover_sandbox_create(requests::create())
        .await
        .unwrap();
    assert_eq!(missing, None);
    let calls = fixture.captured();
    assert_eq!(calls.iter().filter(|call| call.method == "POST").count(), 1);
    assert!(calls.iter().skip(1).all(|call| call.method == "GET"));
}

#[tokio::test]
async fn restoration_named_none_cannot_recover_a_create_without_a_source() {
    let fixture = Fixture::start().await;
    let mut request = requests::create();
    fixture
        .backend
        .create_sandbox(request.clone())
        .await
        .unwrap();
    request.snapshot_provider_ref = Some(ProviderRef::new("none"));
    assert!(
        matches!(
            fixture.backend.recover_sandbox_create(request).await,
            Err(Error::Internal(_))
        ),
        "an absent snapshot and a saved snapshot named none need different request identities"
    );
}

#[tokio::test]
async fn recovery_fails_closed_on_each_conflicting_request_identity_and_duplicates() {
    let fixture = Fixture::start().await;
    let request = requests::create();
    let created = fixture
        .backend
        .create_sandbox(request.clone())
        .await
        .unwrap();
    let original = fixture.state.lock().unwrap().sandboxes[created.provider_ref.as_str()].clone();
    let keys = [
        "sandbox.id",
        "sandbox.workspace",
        "sandbox.agent",
        "sandbox.consumer",
        "sandbox.profile",
        "sandbox.requested-size",
        "sandbox.size",
        "sandbox.lifetime",
        "sandbox.ttl",
        "sandbox.from",
        "sandbox.deployment",
        "sandbox.backend",
    ];
    for key in keys {
        let mut row = original.clone();
        row["labels"][key] = "conflict".into();
        fixture
            .state
            .lock()
            .unwrap()
            .sandboxes
            .insert(created.provider_ref.as_str().to_owned(), row);
        assert!(
            matches!(
                fixture
                    .backend
                    .recover_sandbox_create(request.clone())
                    .await,
                Err(Error::Internal(_))
            ),
            "{key}"
        );
    }
    let mut row = original.clone();
    row["labels"]["sandbox.network"] = "allowlist".into();
    fixture
        .state
        .lock()
        .unwrap()
        .sandboxes
        .insert(created.provider_ref.as_str().to_owned(), row);
    assert!(matches!(
        fixture
            .backend
            .recover_sandbox_create(request.clone())
            .await,
        Err(Error::SandboxNetworkPolicyMismatch)
    ));
    let mut duplicate = original.clone();
    duplicate["id"] = "bx_33333333".into();
    {
        let mut state = fixture.state.lock().unwrap();
        state
            .sandboxes
            .insert(created.provider_ref.as_str().to_owned(), original);
        state.sandboxes.insert("bx_33333333".to_owned(), duplicate);
    }
    assert!(matches!(
        fixture.backend.recover_sandbox_create(request).await,
        Err(Error::Internal(_))
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
async fn paginated_inventory_keeps_recognized_metadata_and_leaves_legacy_unknown() {
    let fixture = Fixture::start().await;
    let request = requests::create();
    let created = fixture
        .backend
        .create_sandbox(request.clone())
        .await
        .unwrap();
    {
        let mut state = fixture.state.lock().unwrap();
        state.page_size = 1;
        state.sandboxes.insert("bx_33333333".to_owned(), state::sandbox("bx_33333333", json!({
            "sandbox.deployment":"fixture-deployment","sandbox.id":"bad-id","sandbox.consumer":"unknown",
            "sandbox.lifetime":"one_shot","sandbox.ttl":"21600.0"
        }), false));
    }
    let rows = fixture
        .backend
        .list_managed_sandboxes(request.deployment_id)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    let current = rows
        .iter()
        .find(|row| row.provider_ref == created.provider_ref)
        .unwrap();
    assert_eq!(current.sandbox_id, Some(request.sandbox_id));
    assert_eq!(current.operation_id, Some(request.operation_id));
    assert_eq!(current.consumer, Some(request.consumer));
    assert_eq!(current.lifetime, Some(request.lifetime));
    let legacy = rows
        .iter()
        .find(|row| row.provider_ref != created.provider_ref)
        .unwrap();
    assert_eq!(
        (
            legacy.sandbox_id,
            legacy.operation_id,
            legacy.consumer,
            legacy.lifetime
        ),
        (None, None, None, None)
    );
    assert!(fixture.captured().last().unwrap().path.contains("cursor=1"));
    assert!(matches!(
        current.lifetime,
        Some(SandboxLifetime::OneShot { .. })
    ));
}

#[tokio::test]
async fn malformed_or_repeated_inventory_cannot_prove_recovery() {
    let fixture = Fixture::start().await;
    for row in [
        json!({"id":"..","state":"ready"}),
        json!({"id":"bx_22222222","state":"future-state"}),
    ] {
        fixture.state.lock().unwrap().overrides.push_back(Override {
            method: "GET".to_owned(),
            path:
                "/api/v1/sandboxes?limit=200&sort=asc&label=sandbox.deployment%3Dfixture-deployment"
                    .to_owned(),
            reply: Reply::json(
                200,
                json!({"ok":true,"type":"sandbox.list","sandboxes":[row]}),
            ),
        });
        assert!(matches!(
            fixture
                .backend
                .list_managed_sandboxes("fixture-deployment".to_owned())
                .await,
            Err(Error::BackendUnavailable { .. })
        ));
    }
    let row = state::sandbox(
        "bx_22222222",
        json!({"sandbox.deployment":"fixture-deployment"}),
        false,
    );
    for cursor in [None, Some("repeat")] {
        let suffix = cursor.map_or(String::new(), |cursor| format!("&cursor={cursor}"));
        fixture.state.lock().unwrap().overrides.push_back(Override { method:"GET".to_owned(), path:format!("/api/v1/sandboxes?limit=200&sort=asc&label=sandbox.deployment%3Dfixture-deployment{suffix}"),
            reply:Reply::json(200,json!({"ok":true,"type":"sandbox.list","sandboxes":[row],"pageInfo":{"hasMore":true,"nextCursor":"repeat"}})) });
    }
    assert!(matches!(
        fixture
            .backend
            .list_managed_sandboxes("fixture-deployment".to_owned())
            .await,
        Err(Error::BackendUnavailable { .. })
    ));
}
