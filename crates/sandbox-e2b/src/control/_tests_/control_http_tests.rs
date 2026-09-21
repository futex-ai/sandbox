//! Control-plane HTTP timeout regressions.

use std::{
    io::{ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    thread::JoinHandle,
    time::{Duration, Instant},
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

#[tokio::test]
async fn credentialed_control_client_does_not_follow_redirects() {
    let target = TcpListener::bind("127.0.0.1:0").expect("bind redirect target");
    let target_address = target.local_addr().expect("redirect target address");
    target
        .set_nonblocking(true)
        .expect("nonblocking redirect target");
    let target = std::thread::spawn(move || capture_redirect_target(target));
    let (api_base, redirect) = redirect_server(format!("http://{target_address}/stolen"));
    let transport = ReqwestE2bHttpTransport::new_with_timeouts(
        api_base,
        "api-key".to_owned(),
        Duration::from_secs(1),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .expect("redirect-test transport");

    let response = transport
        .send(request(Method::Get, false))
        .await
        .expect("redirect response should remain local");

    assert_eq!(response.status, 302);
    redirect.join().expect("redirect server should finish");
    assert!(
        target
            .join()
            .expect("redirect target should finish")
            .is_none(),
        "credentialed client followed a cross-origin redirect"
    );
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

fn redirect_server(location: String) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind redirect server");
    let address = listener.local_addr().expect("redirect server address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept redirect request");
        let request = read_request_headers(&mut stream);
        assert!(
            String::from_utf8_lossy(&request)
                .to_ascii_lowercase()
                .contains("x-api-key: api-key")
        );
        write!(
            stream,
            "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
        .expect("write redirect response");
    });
    (format!("http://{address}"), server)
}

fn capture_redirect_target(listener: TcpListener) -> Option<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let request = read_request_headers(&mut stream);
                stream
                    .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                    .expect("write redirect target response");
                return Some(request);
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => return None,
            Err(error) => panic!("accept redirect target: {error}"),
        }
    }
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
