//! Exhaustive status mapping tests for envd Connect requests.

use crate::{E2bAdapterError, ProcessConnection};

use super::{ReqwestConnectHttpTransport, map_status, server_streaming_request_body};

#[test]
fn server_streaming_requests_frame_one_json_message() {
    let request = br#"{"process":{"cmd":"/bin/true"}}"#;

    let body = server_streaming_request_body(request).expect("request body");

    assert_eq!(body[0], 0);
    assert_eq!(
        u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize,
        request.len()
    );
    assert_eq!(&body[5..], request);
}

#[test]
fn connect_requests_use_the_sandbox_specific_envd_host() {
    let transport = ReqwestConnectHttpTransport::new().expect("transport");
    let connection = ProcessConnection::new(
        "sandbox-id".to_owned(),
        "e2b.app".to_owned(),
        "access-token".to_owned(),
    );

    let request = transport
        .request(&connection, "List", "application/json")
        .build()
        .expect("request");

    assert_eq!(
        request.url().as_str(),
        "https://49983-sandbox-id.e2b.app/process.Process/List"
    );
    assert!(request.headers().contains_key("X-Access-Token"));
    assert_eq!(
        request.headers()["User-Agent"],
        concat!("sandbox-e2b/", env!("CARGO_PKG_VERSION"))
    );
    assert!(!request.headers().contains_key("E2b-Sandbox-Id"));
    assert!(!request.headers().contains_key("E2b-Sandbox-Port"));
}

#[test]
fn connect_status_mapping_is_typed_and_bounded() {
    for status in [200, 201, 204, 299] {
        assert!(map_status(status, false).is_ok());
    }
    for status in [400, 418, 600] {
        assert!(matches!(
            map_status(status, false),
            Err(E2bAdapterError::InvalidRequest)
        ));
    }
    for status in [401, 403] {
        assert!(matches!(
            map_status(status, false),
            Err(E2bAdapterError::Unauthorized)
        ));
    }
    assert!(matches!(
        map_status(404, false),
        Err(E2bAdapterError::NotFound)
    ));
    for status in [408, 429, 500, 599] {
        assert!(matches!(
            map_status(status, false),
            Err(E2bAdapterError::Unavailable)
        ));
        assert!(matches!(
            map_status(status, true),
            Err(E2bAdapterError::DeliveryAmbiguous)
        ));
    }
}
