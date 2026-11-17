//! Create distinguishes proven local rejection from uncertain provider delivery.

use std::sync::Arc;

use sandbox_boat::{
    BoatAdapterError, BoatSandboxBackend,
    control::{
        api::{BoatControlApi, BoatControlApiMock},
        client::ReqwestBoatControlApi,
        types::BoatCreateSandbox,
    },
    transport::http::BoatHttpTransportMock,
};
use sandbox_interface::{Error, SandboxLifecycle};
use unimock::{MockFn, Unimock, matching};

use super::support::{fixture, requests};

#[tokio::test]
async fn control_pre_dispatch_rejections_keep_their_typed_cause() {
    let control = ReqwestBoatControlApi::with_transport(Arc::new(Unimock::new(())));
    let mut request = control_request();
    request.from = Some("invalid_plain_name".to_owned());
    let result = control.create_sandbox(request).await;
    assert!(
        matches!(result, Err(BoatAdapterError::CreateNotSent { ref source })
            if matches!(source.as_ref(), BoatAdapterError::InvalidRequest)),
        "invalid create must prove that no request was sent"
    );

    let control = ReqwestBoatControlApi::with_transport(Arc::new(Unimock::new(
        BoatHttpTransportMock::send
            .some_call(matching!(_))
            .returns(Err(BoatAdapterError::InvalidRequest)),
    )));
    let result = control.create_sandbox(control_request()).await;
    assert!(
        matches!(result, Err(BoatAdapterError::CreateNotSent { ref source })
            if matches!(source.as_ref(), BoatAdapterError::InvalidRequest)),
        "transport validation must keep its no-send guarantee"
    );
}

#[tokio::test]
async fn backend_create_distinguishes_unsent_control_rejection_from_uncertain_delivery() {
    for (failure, unsent) in [
        (BoatAdapterError::InvalidRequest, true),
        (
            BoatAdapterError::CreateNotSent {
                source: Box::new(BoatAdapterError::InvalidRequest),
            },
            true,
        ),
        (BoatAdapterError::DeliveryAmbiguous, false),
    ] {
        let control = Unimock::new(
            BoatControlApiMock::create_sandbox
                .some_call(matching!(_))
                .returns(Err(failure)),
        );
        let backend = BoatSandboxBackend::with_clients(
            fixture::config("https://example.test"),
            Arc::new(control),
            Arc::new(Unimock::new(())),
        );
        let result = backend.create_sandbox(requests::create()).await;
        assert_eq!(
            matches!(&result, Err(Error::SandboxCreateNotSent { .. })),
            unsent,
            "create must preserve its dispatch certainty: {result:?}"
        );
        assert!(
            matches!(result,
                Err(Error::SandboxCreateNotSent { ref source })
                    if matches!(source.as_ref(), Error::Internal(_)))
                || matches!(result, Err(Error::BackendUnavailable { .. })),
            "create must retain a typed local cause or an uncertain result"
        );
    }
}

fn control_request() -> BoatCreateSandbox {
    BoatCreateSandbox {
        machine_type: "default".to_owned(),
        ttl_seconds: 21_600,
        no_env: true,
        snapshots: false,
        monitoring: false,
        labels: Default::default(),
        from: None,
        operation: "synthetic-create-operation".to_owned(),
    }
}
