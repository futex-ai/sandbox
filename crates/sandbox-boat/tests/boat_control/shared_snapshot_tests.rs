//! Shared snapshot restoration preserves secrets and exact recovery identity.

use sandbox_interface::{Error, ProviderRef, SandboxLifecycle, SandboxState};

use super::support::{fixture::Fixture, requests};

const SHARED_SOURCE: &str =
    "bsh_SYNTHETIC_Share-Reference_0123456789_ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdef";

#[tokio::test]
async fn shared_snapshot_restore_preserves_source_without_exposing_recovery_labels() {
    let fixture = Fixture::start().await;
    let mut request = requests::create();
    request.snapshot_provider_ref = Some(ProviderRef::new(SHARED_SOURCE));
    assert!(!format!("{request:?}").contains(SHARED_SOURCE));

    let result = fixture.backend.create_sandbox(request.clone()).await;
    assert!(
        matches!(&result, Ok(created) if created.state == SandboxState::Ready),
        "a shared reference must reach create unchanged: {result:?}"
    );
    let created = result.unwrap();
    let calls = fixture.captured();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].body["from"], SHARED_SOURCE);
    assert_eq!(calls[0].body["noEnv"], true);
    assert_eq!(calls[0].body["snapshots"], false);
    assert_eq!(calls[0].body["monitoring"], false);
    assert!(!calls[0].body["labels"].to_string().contains(SHARED_SOURCE));
    assert_eq!(
        calls[0].body["labels"]["sandbox.from"],
        "shared:sha256:95681ce9bce5ff57511a0a3c555f201bfff03e0b05347be303a7284843a3ce3c"
    );
    assert_eq!(
        fixture
            .backend
            .recover_sandbox_create(request.clone())
            .await
            .unwrap(),
        Some(created)
    );
    request.snapshot_provider_ref = Some(ProviderRef::new("bsh_SYNTHETIC_other-reference"));
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
