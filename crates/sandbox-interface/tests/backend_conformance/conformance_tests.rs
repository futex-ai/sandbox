//! Shared backend conformance-harness coverage.

use sandbox_interface::conformance::exercise_backend;

use super::alternate_backend::AlternateBackend;

#[tokio::test]
async fn asynchronous_alternate_backend_satisfies_the_shared_conformance_harness() {
    exercise_backend(&AlternateBackend::default(), "alternate")
        .await
        .expect("alternate backend should conform");
}
