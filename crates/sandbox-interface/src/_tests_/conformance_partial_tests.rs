//! Snapshot-only conformance preserves a caller-owned source after failure.

use std::sync::{Arc, Mutex};

use unimock::{MockFn, Unimock, matching};

use crate::{
    BackendSnapshot, BackendSnapshotCreateOutcome, BackendSnapshotInventory,
    BackendSnapshotRecovery, Error, ProviderRef, SandboxSnapshotsMock, SnapshotState,
    conformance::exercise_snapshots,
};

#[tokio::test]
async fn snapshot_probe_recovers_and_cleans_snapshot_after_inspection_failure() {
    let deleted = Arc::new(Mutex::new(Vec::new()));
    let snapshots = Unimock::new((
        SandboxSnapshotsMock::snapshot_inventory
            .some_call(matching!(_, _))
            .answers(&|_, source, _| {
                assert_eq!(source, ProviderRef::new("caller-owned-source"));
                Ok(BackendSnapshotInventory::default())
            }),
        SandboxSnapshotsMock::create_snapshot
            .some_call(matching!(_))
            .answers(&|_, request| {
                assert_eq!(
                    request.source_provider_ref,
                    ProviderRef::new("caller-owned-source")
                );
                Ok(BackendSnapshotCreateOutcome::Created(snapshot()))
            }),
        SandboxSnapshotsMock::recover_snapshot_create
            .some_call(matching!(_))
            .answers(&|_, request| {
                assert_eq!(
                    request.source_provider_ref,
                    ProviderRef::new("caller-owned-source")
                );
                Ok(BackendSnapshotRecovery::Recovered(snapshot()))
            }),
        SandboxSnapshotsMock::inspect_snapshot
            .some_call(matching!(_))
            .answers(&|_, request| {
                assert_eq!(request.provider_ref, ProviderRef::new("owned-snapshot"));
                assert_eq!(
                    request.source_provider_ref,
                    ProviderRef::new("caller-owned-source")
                );
                Err(Error::BackendUnavailable {
                    backend_id: "snapshot-only".to_owned(),
                })
            }),
        SandboxSnapshotsMock::delete_snapshot
            .each_call(matching!(_))
            .answers_arc({
                let deleted = deleted.clone();
                Arc::new(move |_, provider_ref| {
                    deleted.lock().expect("deletions lock").push(provider_ref);
                    Ok(())
                })
            })
            .at_least_times(0),
    ));

    let result = exercise_snapshots(&snapshots, ProviderRef::new("caller-owned-source")).await;

    assert!(
        matches!(result, Err(Error::BackendUnavailable { backend_id }) if backend_id == "snapshot-only")
    );
    assert_eq!(
        *deleted.lock().expect("deletions lock"),
        [ProviderRef::new("owned-snapshot")]
    );
}

fn snapshot() -> BackendSnapshot {
    BackendSnapshot {
        provider_ref: ProviderRef::new("owned-snapshot"),
        state: SnapshotState::Ready,
    }
}
