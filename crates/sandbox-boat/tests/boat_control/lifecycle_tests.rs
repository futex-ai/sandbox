//! Direct start, local validation, non-mutating access, and deletion contracts.

use std::time::Duration;

use sandbox_interface::{
    EgressDestination, Error, ProviderRef, SandboxConsumer, SandboxLifecycle, SandboxLifetime,
    SandboxNetworkPolicy, SandboxSizeSelection, SandboxState,
};

use super::support::{fixture::Fixture, requests};

#[tokio::test]
async fn create_dispatches_once_with_disposable_settings_and_read_only_access() {
    let fixture = Fixture::start().await;
    let request = requests::create();
    let created = fixture
        .backend
        .create_sandbox(request.clone())
        .await
        .unwrap();
    assert_eq!(created.state, SandboxState::Ready);
    let calls = fixture.captured();
    assert_eq!(
        calls.len(),
        1,
        "create must not preflight inventory or limits"
    );
    assert_eq!(calls[0].method, "POST");
    assert_eq!(calls[0].path, "/api/v1/sandboxes");
    assert_eq!(
        calls[0].headers["idempotency-key"],
        request.operation_id.to_string()
    );
    assert_eq!(calls[0].headers["authorization"], "Bearer fixture-key");
    assert_eq!(calls[0].headers["x-boat-org"], "personal");
    assert_eq!(calls[0].body["noEnv"], true);
    assert_eq!(calls[0].body["snapshots"], false);
    assert_eq!(calls[0].body["monitoring"], false);
    assert_eq!(calls[0].body.get("env"), None);
    assert!(!calls[0].body.to_string().contains("fixture-key"));
    assert_eq!(calls[0].body["type"], "default");
    assert_eq!(calls[0].body["ttlSeconds"], 21_600);
    assert_eq!(
        calls[0].body["labels"]["sandbox.workspace"],
        request.owner.workspace_id.to_string()
    );
    assert_eq!(calls[0].body["labels"]["sandbox.consumer"], "runtime");
    assert_eq!(calls[0].body["labels"]["sandbox.network"], "open");
    assert_eq!(calls[0].body["labels"]["sandbox.lifetime"], "one_shot");
    let inspected = fixture
        .backend
        .inspect_sandbox(created.provider_ref.clone())
        .await
        .unwrap();
    assert_eq!(inspected, created);
    fixture
        .state
        .lock()
        .unwrap()
        .sandboxes
        .get_mut(created.provider_ref.as_str())
        .unwrap()["state"] = "archived".into();
    assert_eq!(
        fixture
            .backend
            .inspect_sandbox(created.provider_ref.clone())
            .await
            .unwrap()
            .state,
        SandboxState::Destroyed
    );
    assert_eq!(
        fixture.state.lock().unwrap().sandboxes[created.provider_ref.as_str()]["ttlSeconds"],
        21_600
    );
    assert_eq!(
        fixture
            .captured()
            .iter()
            .skip(1)
            .map(|call| call.method.as_str())
            .collect::<Vec<_>>(),
        ["GET", "GET"]
    );
}

#[tokio::test]
async fn unsupported_network_size_lifetime_and_source_fail_before_dispatch() {
    let fixture = Fixture::start().await;
    let mut network = requests::create();
    network.network =
        SandboxNetworkPolicy::allowlist(vec![EgressDestination::domain("example.com").unwrap()])
            .unwrap();
    assert!(matches!(
        fixture.backend.create_sandbox(network.clone()).await,
        Err(Error::UnsupportedNetworkPolicy { .. })
    ));
    assert!(matches!(
        fixture.backend.recover_sandbox_create(network).await,
        Err(Error::UnsupportedNetworkPolicy { .. })
    ));
    let mut size = requests::create();
    size.size = Some(SandboxSizeSelection::ProviderSize("unknown".to_owned()));
    assert!(matches!(
        fixture.backend.create_sandbox(size).await,
        Err(Error::UnsupportedSizeSelection { .. })
    ));
    for duration in [
        Duration::ZERO,
        Duration::from_millis(1500),
        Duration::from_secs(2_592_001),
    ] {
        let mut request = requests::create();
        request.lifetime = SandboxLifetime::OneShot {
            max_lifetime: duration,
        };
        assert!(
            matches!(
                fixture.backend.create_sandbox(request).await,
                Err(Error::InvalidSeconds { .. })
            ),
            "{duration:?}"
        );
    }
    let mut interactive = requests::create();
    interactive.lifetime = SandboxLifetime::IdleAutoPause;
    assert!(matches!(
        fixture.backend.create_sandbox(interactive).await,
        Err(Error::Internal(_))
    ));
    let mut browser = requests::create();
    browser.consumer = SandboxConsumer::Browser;
    browser.snapshot_provider_ref = Some(ProviderRef::new("capture-name"));
    assert!(matches!(
        fixture.backend.create_sandbox(browser).await,
        Err(Error::RuntimeConsumerRequired { .. })
    ));
    assert_eq!(fixture.captured().len(), 0);
    let mut maximum = requests::create();
    maximum.lifetime = SandboxLifetime::OneShot {
        max_lifetime: Duration::from_secs(2_592_000),
    };
    fixture.backend.create_sandbox(maximum).await.unwrap();
    assert_eq!(fixture.captured()[0].body["ttlSeconds"], 2_592_000);
}

#[tokio::test]
async fn deletion_repeats_scoped_delete_until_completed_then_accepts_absence() {
    let fixture = Fixture::start().await;
    let created = fixture
        .backend
        .create_sandbox(requests::create())
        .await
        .unwrap();
    fixture.state.lock().unwrap().deletion_statuses =
        ["processing".to_owned(), "completed".to_owned()].into();
    fixture
        .backend
        .destroy_sandbox(created.provider_ref.clone())
        .await
        .unwrap();
    fixture
        .backend
        .destroy_sandbox(created.provider_ref.clone())
        .await
        .unwrap();
    let calls = fixture.captured();
    let deletes = calls
        .iter()
        .filter(|call| call.method == "DELETE")
        .collect::<Vec<_>>();
    assert_eq!(
        deletes.len(),
        3,
        "a pending delete cannot count as completion"
    );
    assert!(
        deletes
            .iter()
            .all(|call| call.headers["x-ascii-confirm-delete"] == created.provider_ref.as_str())
    );
    assert_eq!(
        calls
            .iter()
            .filter(|call| call.path.contains("deletion-operations"))
            .count(),
        0
    );
    assert!(
        !fixture
            .state
            .lock()
            .unwrap()
            .sandboxes
            .contains_key(created.provider_ref.as_str())
    );
}
