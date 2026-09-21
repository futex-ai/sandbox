//! Exhaustive status mapping tests for E2B control requests.

use crate::E2bAdapterError;

use super::{ensure_status, path_segment};

#[test]
fn accepted_control_statuses_pass_through() {
    assert!(ensure_status(200, &[200, 201], false).is_ok());
    assert!(ensure_status(201, &[200, 201], true).is_ok());
}

#[test]
fn handled_control_statuses_map_without_response_payloads() {
    for status in [400, 409, 422, 418] {
        assert!(matches!(
            ensure_status(status, &[], false),
            Err(E2bAdapterError::InvalidRequest)
        ));
    }
    for status in [401, 403] {
        assert!(matches!(
            ensure_status(status, &[], false),
            Err(E2bAdapterError::Unauthorized)
        ));
    }
    assert!(matches!(
        ensure_status(404, &[], false),
        Err(E2bAdapterError::NotFound)
    ));
    for status in [408, 429, 500, 599] {
        assert!(matches!(
            ensure_status(status, &[], false),
            Err(E2bAdapterError::Unavailable)
        ));
        assert!(matches!(
            ensure_status(status, &[], true),
            Err(E2bAdapterError::DeliveryAmbiguous)
        ));
    }
}

#[test]
fn opaque_path_segments_cannot_escape_provider_routes() {
    assert_eq!(
        path_segment("team/snapshot:tag?x=1"),
        "team%2Fsnapshot%3Atag%3Fx%3D1"
    );
}
