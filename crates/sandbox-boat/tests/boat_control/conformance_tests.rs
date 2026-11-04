//! Shared supported probes use production HTTP and a caller-owned capture source.

use std::time::Duration;

use sandbox_interface::{
    Error, ProviderRef, SandboxLifetime, SnapshotState,
    conformance::{exercise_lifecycle, exercise_one_shot_lifetime, exercise_snapshots},
};

use super::support::{fixture::Fixture, state::CAPTURE_SOURCE};

#[tokio::test]
async fn supported_conformance_probes_preserve_identity_cleanup_and_source_ownership() {
    let fixture = Fixture::start().await;
    exercise_lifecycle(
        &fixture.backend,
        "general",
        SandboxLifetime::OneShot {
            max_lifetime: Duration::from_secs(21_600),
        },
    )
    .await
    .unwrap();
    exercise_one_shot_lifetime(&fixture.backend, "general")
        .await
        .unwrap();
    exercise_snapshots(&fixture.backend, ProviderRef::new(CAPTURE_SOURCE))
        .await
        .unwrap();
    let state = fixture.state.lock().unwrap();
    assert_eq!(
        state
            .sandboxes
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [CAPTURE_SOURCE]
    );
    assert_eq!(state.snapshots.len(), 0);
    assert_eq!(
        state
            .requests
            .iter()
            .filter(|call| call.method == "POST" && call.path == "/api/v1/sandboxes")
            .count(),
        2
    );
    assert_eq!(
        state
            .requests
            .iter()
            .filter(|call| call.method == "POST" && call.path == "/api/v1/saved-snapshots")
            .count(),
        1
    );
}

#[tokio::test]
async fn failed_capture_keeps_known_identity_for_cleanup_and_never_reports_success() {
    let fixture = Fixture::start().await;
    fixture.state.lock().unwrap().snapshot_status = "failed".to_owned();
    let result = exercise_snapshots(&fixture.backend, ProviderRef::new(CAPTURE_SOURCE)).await;
    let state = fixture.state.lock().unwrap();
    assert_eq!(
        state.snapshots.len(),
        0,
        "a failed named capture must still be deleted"
    );
    assert!(matches!(
        result,
        Err(Error::SnapshotStateConflict {
            state: SnapshotState::Failed
        })
    ));
    assert!(state.sandboxes.contains_key(CAPTURE_SOURCE));
    assert_eq!(
        state
            .requests
            .iter()
            .filter(
                |call| call.method == "DELETE" && call.path.starts_with("/api/v1/saved-snapshots/")
            )
            .count(),
        1
    );
}
