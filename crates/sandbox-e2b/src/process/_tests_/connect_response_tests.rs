//! Connect unary response classification regressions.

use std::sync::Arc;

use sandbox_interface::Error as DomainError;
use unimock::{MockFn, Unimock, matching};

use crate::{ProcessConnection, ProcessSelector, ProcessTransport};

use super::{ConnectProcessTransport, EmptyWire, ListResponseWire};
use crate::{
    error::Error,
    process::http::{ConnectHttpTransport, unary as unary_call},
};

impl ConnectProcessTransport {
    fn with_response_http(http: Arc<dyn ConnectHttpTransport>) -> Self {
        Self {
            http,
            backend_id: "configured-e2b".to_owned(),
        }
    }
}

#[tokio::test]
async fn malformed_safe_unary_response_is_provider_unavailable() {
    let transport = transport(false);

    let result = transport.list(connection()).await;

    assert!(matches!(
        result,
        Err(DomainError::BackendUnavailable { backend_id })
            if backend_id == "configured-e2b"
    ));
}

#[tokio::test]
async fn malformed_mutation_response_preserves_delivery_ambiguity() {
    let transport = transport(true);

    let result: Result<ListResponseWire, Error> = transport
        .unary(connection(), "Mutation", &EmptyWire {}, true)
        .await;

    assert!(matches!(result, Err(Error::DeliveryAmbiguous)));
}

#[tokio::test]
async fn malformed_empty_mutation_response_preserves_delivery_ambiguity() {
    let transport = transport(true);

    let result = transport
        .send_input(connection(), ProcessSelector::Pid(7), b"input".to_vec())
        .await;

    assert!(matches!(result, Err(DomainError::DeliveryUnknown)));
}

fn transport(ambiguous: bool) -> ConnectProcessTransport {
    let http: Arc<dyn ConnectHttpTransport> = Arc::new(Unimock::new(
        unary_call
            .next_call(matching!(_, _, _, _))
            .answers_arc(Arc::new(move |_, _, _, _, actual| {
                assert_eq!(actual, ambiguous);
                Ok(b"not-json".to_vec())
            })),
    ));
    ConnectProcessTransport::with_response_http(http)
}

fn connection() -> ProcessConnection {
    ProcessConnection::new(
        "sandbox".to_owned(),
        "e2b.app".to_owned(),
        "access-token".to_owned(),
    )
}
