//! Control-plane response error-classification regressions.

use std::{
    io::{Read, Write},
    net::TcpListener,
    time::Duration,
};

use crate::E2bAdapterError;

use super::request_error;

#[tokio::test]
async fn truncated_response_bodies_are_retryable_provider_failures() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind body endpoint");
    let address = listener.local_addr().expect("body endpoint address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept body request");
        let mut request = Vec::new();
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let mut chunk = [0_u8; 1024];
            let read = stream.read(&mut chunk).expect("read body request");
            assert_ne!(read, 0, "client closed before its request headers");
            request.extend_from_slice(&chunk[..read]);
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nx")
            .expect("write truncated response");
        std::thread::sleep(Duration::from_millis(50));
    });
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .expect("test client")
        .get(format!("http://{address}/body"))
        .send()
        .await
        .expect("response headers");
    let error = response
        .bytes()
        .await
        .expect_err("short body should fail collection");
    server.join().expect("body server should finish");

    assert!(matches!(
        request_error(error, false, "test body"),
        E2bAdapterError::Unavailable
    ));
}
