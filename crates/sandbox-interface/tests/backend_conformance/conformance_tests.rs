//! Shared backend conformance-harness coverage.

use sandbox_interface::conformance::{exercise_backend, exercise_one_shot_lifetime};

use super::alternate_backend::AlternateBackend;

#[tokio::test]
async fn asynchronous_alternate_backend_satisfies_the_shared_conformance_harness() {
    exercise_backend(&AlternateBackend::default(), "alternate")
        .await
        .expect("alternate backend should conform");
}

#[tokio::test]
async fn one_shot_probe_creates_and_destroys_a_bounded_sandbox() {
    let backend = AlternateBackend::default();

    exercise_one_shot_lifetime(&backend, "alternate")
        .await
        .expect("one-shot lifetime probe should conform");

    assert!(backend.faults.one_shot_create_was_exercised());
    assert!(backend.faults.cleanup_attempted_for_every_resource());
}

#[tokio::test]
async fn ambiguous_sandbox_creation_is_recovered_without_redispatch() {
    let backend = AlternateBackend::default();
    backend.faults.make_next_create_ambiguous();

    exercise_backend(&backend, "alternate")
        .await
        .expect("an accepted create with an ambiguous response should recover");
}

#[tokio::test]
async fn eventually_consistent_sandbox_recovery_is_polled() {
    let backend = AlternateBackend::default();
    backend.faults.set_recovery_misses(1);

    exercise_backend(&backend, "alternate")
        .await
        .expect("one empty recovery inventory should be retried");
}

#[tokio::test]
async fn cleanup_is_attempted_for_every_resource_after_a_failure() {
    let backend = AlternateBackend::default();
    backend.faults.fail_snapshot_deletes();

    exercise_backend(&backend, "alternate")
        .await
        .expect_err("the injected snapshot cleanup failure should be preserved");

    assert!(backend.faults.cleanup_attempted_for_every_resource());
}

#[tokio::test]
async fn cleanup_continues_after_terminal_close_failure() {
    let backend = AlternateBackend::default();
    backend.faults.fail_terminal_closes();

    exercise_backend(&backend, "alternate")
        .await
        .expect_err("the injected terminal cleanup failure should be preserved");

    assert!(backend.faults.cleanup_attempted_for_every_resource());
}
