//! Exhaustive request construction and status mapping tests for envd Connect.

use std::time::Duration;

use crate::{
    E2bAdapterError, ProcessConnection,
    process::http_stream::{request as streaming_request, server_streaming_request_body},
};

use super::{ReqwestConnectHttpTransport, map_status};

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
fn streaming_request_uses_the_caller_owned_transport_timeout() {
    let transport = ReqwestConnectHttpTransport::new().expect("transport");
    let connection = ProcessConnection::new(
        "sandbox-id".to_owned(),
        "e2b.app".to_owned(),
        "access-token".to_owned(),
    );
    let timeout = Duration::from_secs(3610);

    let request = streaming_request(
        &transport,
        &connection,
        "Start",
        br#"{"process":{"cmd":"/bin/true"}}"#,
        Some(timeout),
    )
    .expect("streaming request")
    .build()
    .expect("built request");

    assert_eq!(request.timeout(), Some(&timeout));
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
        .expect("validated envd URL")
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
fn process_and_file_requests_authenticate_an_explicit_process_user() {
    let transport = ReqwestConnectHttpTransport::new().expect("transport");
    let connection = ProcessConnection::new(
        "sandbox-id".to_owned(),
        "e2b.app".to_owned(),
        "access-token".to_owned(),
    )
    .with_user("root");

    let process_request = transport
        .request(&connection, "Start", "application/connect+json")
        .expect("validated envd URL")
        .build()
        .expect("request");
    let file_request = transport
        .file_request(&connection, reqwest::Method::GET, "/tmp/file")
        .expect("validated envd URL")
        .build()
        .expect("file request");

    assert_eq!(process_request.headers()["Authorization"], "Basic cm9vdDo=");
    assert_eq!(file_request.headers()["Authorization"], "Basic cm9vdDo=");
}

#[test]
fn connect_requests_reject_authority_injection_before_adding_a_token() {
    let transport = ReqwestConnectHttpTransport::new().expect("transport");
    for sandbox_id in [
        "sandbox@attacker.example/",
        "sandbox/path",
        "sandbox#fragment",
    ] {
        let connection = ProcessConnection::new(
            sandbox_id.to_owned(),
            "e2b.app".to_owned(),
            "access-token".to_owned(),
        );

        assert!(matches!(
            transport.request(&connection, "List", "application/json"),
            Err(E2bAdapterError::InvalidRequest)
        ));
        assert!(matches!(
            transport.file_request(&connection, reqwest::Method::GET, "/tmp/file"),
            Err(E2bAdapterError::InvalidRequest)
        ));
    }
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
