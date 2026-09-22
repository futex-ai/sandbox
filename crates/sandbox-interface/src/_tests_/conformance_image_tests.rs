//! Image conformance source-recovery and cleanup regressions.

use std::sync::{Arc, Mutex};

use crate::{
    BackendPreparedImage, BackendSandbox, Error, ProviderRef, SandboxBackendMock, SandboxState,
};
use unimock::{MockFn, Unimock, matching};
use uuid::Uuid;

use super::{exercise, exercise_failed_preparation};

#[tokio::test]
async fn preparation_transport_failure_cleans_the_created_source() {
    let backend = Unimock::new((
        SandboxBackendMock::create_sandbox
            .next_call(matching!(_))
            .returns(Ok(sandbox("prepared-source"))),
        SandboxBackendMock::recover_sandbox_create
            .next_call(matching!(_))
            .returns(Ok(Some(sandbox("prepared-source")))),
        SandboxBackendMock::prepare_image
            .next_call(matching!(_))
            .returns(Err(Error::BackendUnavailable {
                backend_id: "test".to_owned(),
            })),
        SandboxBackendMock::destroy_sandbox
            .next_call(matching!(_))
            .returns(Ok(())),
    ));

    let result = exercise(&backend, "test", Uuid::now_v7()).await;

    assert!(matches!(result, Err(Error::BackendUnavailable { .. })));
}

#[tokio::test]
async fn invalid_prepared_image_cleans_the_created_source() {
    let backend = Unimock::new((
        SandboxBackendMock::create_sandbox
            .next_call(matching!(_))
            .returns(Ok(sandbox("prepared-source"))),
        SandboxBackendMock::recover_sandbox_create
            .next_call(matching!(_))
            .returns(Ok(Some(sandbox("prepared-source")))),
        SandboxBackendMock::prepare_image
            .next_call(matching!(_))
            .returns(Ok(BackendPreparedImage {
                source_provider_ref: ProviderRef::new("different-source"),
                size_bytes: 4096,
            })),
        SandboxBackendMock::destroy_sandbox
            .next_call(matching!(_))
            .returns(Ok(())),
    ));

    let result = exercise(&backend, "test", Uuid::now_v7()).await;

    assert!(matches!(result, Err(Error::Internal(_))));
}

#[tokio::test]
async fn snapshot_inventory_failure_cleans_the_created_source() {
    let backend = Unimock::new((
        SandboxBackendMock::create_sandbox
            .next_call(matching!(_))
            .returns(Ok(sandbox("prepared-source"))),
        SandboxBackendMock::recover_sandbox_create
            .next_call(matching!(_))
            .returns(Ok(Some(sandbox("prepared-source")))),
        SandboxBackendMock::prepare_image
            .next_call(matching!(_))
            .returns(Ok(BackendPreparedImage {
                source_provider_ref: ProviderRef::new("prepared-source"),
                size_bytes: 4096,
            })),
        SandboxBackendMock::snapshot_inventory
            .next_call(matching!(_, _))
            .returns(Err(Error::BackendUnavailable {
                backend_id: "test".to_owned(),
            })),
        SandboxBackendMock::destroy_sandbox
            .next_call(matching!(_))
            .returns(Ok(())),
    ));

    let result = exercise(&backend, "test", Uuid::now_v7()).await;

    assert!(matches!(result, Err(Error::BackendUnavailable { .. })));
}

#[tokio::test]
async fn uncertain_source_creation_recovers_the_exact_request_before_cleanup() {
    let dispatched = Arc::new(Mutex::new(None));
    let backend = Unimock::new((
        SandboxBackendMock::create_sandbox
            .next_call(matching!(_))
            .answers_arc({
                let dispatched = dispatched.clone();
                Arc::new(move |_, request| {
                    *dispatched.lock().expect("dispatched request lock") = Some(request);
                    Err(Error::BackendUnavailable {
                        backend_id: "test".to_owned(),
                    })
                })
            }),
        SandboxBackendMock::recover_sandbox_create
            .next_call(matching!(_))
            .answers_arc({
                let dispatched = dispatched.clone();
                Arc::new(move |_, request| {
                    assert_eq!(
                        dispatched.lock().expect("recovery request lock").as_ref(),
                        Some(&request)
                    );
                    Ok(Some(sandbox("recovered-source")))
                })
            }),
        SandboxBackendMock::prepare_image
            .next_call(matching!(_))
            .returns(Err(Error::BackendUnavailable {
                backend_id: "test".to_owned(),
            })),
        SandboxBackendMock::destroy_sandbox
            .next_call(matching!(_))
            .answers(&|_, provider_ref| {
                assert_eq!(provider_ref, ProviderRef::new("recovered-source"));
                Ok(())
            }),
    ));

    let result = exercise(&backend, "test", Uuid::now_v7()).await;

    assert!(matches!(result, Err(Error::BackendUnavailable { .. })));
}

#[tokio::test]
async fn intentional_failure_source_creation_recovers_before_preparation() {
    let backend = Unimock::new((
        SandboxBackendMock::create_sandbox
            .next_call(matching!(_))
            .returns(Err(Error::BackendUnavailable {
                backend_id: "test".to_owned(),
            })),
        SandboxBackendMock::recover_sandbox_create
            .next_call(matching!(_))
            .returns(Ok(Some(sandbox("failed-source")))),
        SandboxBackendMock::prepare_image
            .next_call(matching!(_))
            .returns(Err(Error::ImageSetupFailed {
                command: None,
                retained_sandbox: None,
            })),
        SandboxBackendMock::destroy_sandbox
            .next_call(matching!(_))
            .answers(&|_, provider_ref| {
                assert_eq!(provider_ref, ProviderRef::new("failed-source"));
                Ok(())
            }),
    ));

    exercise_failed_preparation(&backend, "test", Uuid::now_v7())
        .await
        .expect("the recovered source should exercise and clean the expected failure");
}

fn sandbox(provider_ref: &str) -> BackendSandbox {
    BackendSandbox {
        provider_ref: ProviderRef::new(provider_ref),
        state: SandboxState::Ready,
    }
}
