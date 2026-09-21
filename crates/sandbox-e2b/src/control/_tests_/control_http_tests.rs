//! Control-plane HTTP timeout regressions.

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread::JoinHandle,
    time::Duration,
};

use crate::E2bAdapterError;

use super::{E2bHttpTransport, HttpRequest, Method, ReqwestE2bHttpTransport, response_limit_error};

#[test]
fn accepted_mutation_body_overflow_preserves_delivery_ambiguity() {
    assert!(matches!(
        response_limit_error(201, true),
        E2bAdapterError::DeliveryAmbiguous
    ));
    assert!(matches!(
        response_limit_error(200, false),
        E2bAdapterError::ResponseTooLarge
    ));
}

#[tokio::test]
async fn response_body_timeout_is_typed_as_unavailable() {
    let (api_base, server) = stalled_server(true);
    let transport = ReqwestE2bHttpTransport::new_with_timeouts(
        api_base,
        "api-key".to_owned(),
        Duration::from_millis(50),
        Duration::from_millis(50),
        Duration::from_millis(50),
    )
    .expect("timeout-configured transport");

    let result = transport.send(request(Method::Get, false)).await;

    assert!(
        matches!(result, Err(E2bAdapterError::Unavailable)),
        "unexpected timeout result: {result:?}"
    );
    server.join().expect("stalled server should finish");
}

#[tokio::test]
async fn mutating_request_timeout_preserves_delivery_ambiguity() {
    let (api_base, server) = stalled_server(false);
    let transport = ReqwestE2bHttpTransport::new_with_timeouts(
        api_base,
        "api-key".to_owned(),
        Duration::from_millis(50),
        Duration::from_millis(50),
        Duration::from_millis(50),
    )
    .expect("timeout-configured transport");

    let result = transport.send(request(Method::Post, true)).await;

    assert!(
        matches!(result, Err(E2bAdapterError::DeliveryAmbiguous)),
        "unexpected timeout result: {result:?}"
    );
    server.join().expect("stalled server should finish");
}

fn request(method: Method, ambiguous_on_failure: bool) -> HttpRequest {
    HttpRequest {
        method,
        path_and_query: "/stalled".to_owned(),
        body: None,
        ambiguous_on_failure,
    }
}

fn stalled_server(send_headers: bool) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let address = listener.local_addr().expect("test server address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept test request");
        let request = read_request_headers(&mut stream);
        let request = String::from_utf8(request).expect("HTTP request headers should be UTF-8");
        assert!(request.to_ascii_lowercase().contains(&format!(
            "user-agent: sandbox-e2b/{}",
            env!("CARGO_PKG_VERSION")
        )));
        if send_headers {
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n")
                .expect("write response headers");
        }
        std::thread::sleep(Duration::from_secs(2));
    });
    (format!("http://{address}"), server)
}

fn read_request_headers(stream: &mut TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        let mut chunk = [0_u8; 1024];
        let read = stream.read(&mut chunk).expect("read test request");
        assert_ne!(read, 0, "client closed before sending request headers");
        request.extend_from_slice(&chunk[..read]);
    }
    request
}
