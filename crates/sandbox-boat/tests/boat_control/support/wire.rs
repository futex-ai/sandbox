//! Minimal deterministic HTTP/1 fixture boundary.

use std::{collections::BTreeMap, sync::Arc};

use tokio::sync::Notify;

use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Clone, Debug)]
pub struct CapturedRequest {
    pub method: String,
    pub path: String,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
}

#[derive(Clone)]
pub struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
    pub headers: Vec<(String, String)>,
    pub stall: bool,
    pub segments: Vec<Segment>,
}

impl Reply {
    pub fn json(status: u16, body: Value) -> Self {
        Self {
            status,
            body: serde_json::to_vec(&body).unwrap(),
            headers: Vec::new(),
            stall: false,
            segments: Vec::new(),
        }
    }
}

pub(super) async fn read(stream: &mut (impl AsyncRead + Unpin)) -> CapturedRequest {
    let mut bytes = Vec::new();
    let header_end = loop {
        let mut chunk = [0; 4096];
        let count = stream.read(&mut chunk).await.unwrap();
        assert_ne!(count, 0, "request ended before headers");
        bytes.extend_from_slice(&chunk[..count]);
        if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let head = std::str::from_utf8(&bytes[..header_end]).unwrap();
    let mut lines = head.split("\r\n");
    let mut start = lines.next().unwrap().split(' ');
    let method = start.next().unwrap().to_owned();
    let path = start.next().unwrap().to_owned();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.to_ascii_lowercase(), value.trim().to_owned()))
        .collect::<BTreeMap<_, _>>();
    let length = headers
        .get("content-length")
        .map_or(0, |value| value.parse::<usize>().unwrap());
    while bytes.len() < header_end + length {
        let mut chunk = [0; 4096];
        let count = stream.read(&mut chunk).await.unwrap();
        assert_ne!(count, 0, "request ended before body");
        bytes.extend_from_slice(&chunk[..count]);
    }
    let body = if length == 0 {
        Value::Null
    } else {
        serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap()
    };
    CapturedRequest {
        method,
        path,
        headers,
        body,
    }
}

pub(super) async fn write(stream: &mut (impl AsyncWrite + Unpin), reply: Reply) {
    let declared_length = reply.body.len()
        + reply.segments.iter().map(|s| s.bytes.len()).sum::<usize>()
        + usize::from(reply.stall);
    let chunked = reply
        .headers
        .iter()
        .any(|(key, value)| key.eq_ignore_ascii_case("transfer-encoding") && value == "chunked");
    let mut head = format!(
        "HTTP/1.1 {} Fixture\r\nContent-Type: application/json\r\nConnection: close\r\n",
        reply.status
    );
    if !chunked {
        head.push_str(&format!("Content-Length: {declared_length}\r\n"));
    }
    for (key, value) in reply.headers {
        head.push_str(&format!("{key}: {value}\r\n"));
    }
    head.push_str("\r\n");
    if stream.write_all(head.as_bytes()).await.is_err() {
        return;
    }
    if chunked
        && stream
            .write_all(format!("{:x}\r\n", reply.body.len()).as_bytes())
            .await
            .is_err()
    {
        return;
    }
    for segment in reply.segments {
        if let Some(wait) = segment.wait {
            wait.notified().await;
        }
        if stream.write_all(&segment.bytes).await.is_err() {
            return;
        }
        if stream.flush().await.is_err() {
            return;
        }
        if let Some(sent) = segment.sent {
            sent.notify_one();
        }
    }
    if stream.write_all(&reply.body).await.is_err() {
        return;
    }
    if chunked && stream.write_all(b"\r\n0\r\n\r\n").await.is_err() {
        return;
    }
    if stream.flush().await.is_err() {
        return;
    }
    if reply.stall {
        std::future::pending::<()>().await;
    }
    let _ = stream.shutdown().await;
}

#[derive(Clone)]
pub struct Segment {
    pub bytes: Vec<u8>,
    pub wait: Option<Arc<Notify>>,
    pub sent: Option<Arc<Notify>>,
}
