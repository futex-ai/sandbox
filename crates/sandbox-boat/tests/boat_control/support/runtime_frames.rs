//! Fragmented nested NDJSON with release and final-ack barriers.

use std::sync::Arc;

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use tokio::sync::Notify;
use url::Url;

use super::{
    runtime_state::{Fault, RuntimeState},
    wire::{Reply, Segment},
};

pub(super) fn attach(state: &mut RuntimeState, url: &Url) -> Reply {
    let process_id = url
        .path()
        .split('/')
        .rev()
        .nth(1)
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let query = url
        .query_pairs()
        .collect::<std::collections::BTreeMap<_, _>>();
    let stdout = query["stdoutOffset"].parse::<u64>().unwrap();
    let stderr = query["stderrOffset"].parse::<u64>().unwrap();
    assert_eq!(stderr, 0);
    let process = state.processes.get_mut(&process_id).unwrap();
    assert_eq!(stdout, process.offset);
    process.connections += 1;
    let attempt = process.attempt.clone();
    let manifest = state.bundles[&attempt]["manifest"].clone();
    let mut output = state.stdout.clone();
    let mut error = state.stderr.clone();
    if let Some(value) = manifest["envs"]["SANDBOX_PROBE"].as_str() {
        output = format!("/workspace\n{value}").into_bytes();
        error = if value == "environment-map" {
            b"separate-stderr".to_vec()
        } else {
            b"stream-stderr".to_vec()
        };
    }
    let wrong = "ffffffffffffffffffffffffffffffff";
    let identity = if matches!(state.fault, Fault::WrongAttempt) {
        wrong
    } else {
        &attempt
    };
    let mut inner = Vec::new();
    inner.extend(frame(identity, "hello", json!({})));
    inner.extend(frame(identity, "started", json!({"pid":2001})));
    if matches!(state.fault, Fault::Backpressure) {
        for _ in 0..1024 {
            inner.extend(frame(identity, "stdout", json!({"data":"eA=="})));
        }
    } else {
        for (kind, bytes) in [("stdout", output), ("stderr", error)] {
            for chunk in bytes.chunks(16384) {
                inner.extend(frame(
                    identity,
                    kind,
                    json!({"data":STANDARD.encode(chunk)}),
                ));
            }
        }
    }
    let prefix = inner.len();
    if matches!(state.fault, Fault::MalformedThenEnd) {
        inner.extend(b"{\"kind\":\"wrong\"}\n");
    }
    if !matches!(state.fault, Fault::HoldAfterPid | Fault::Backpressure) {
        inner.extend(frame(
            identity,
            "exited",
            json!({"exit_code":state.code,"exited":state.exited}),
        ));
        if !matches!(state.fault, Fault::MissingSuccess) {
            inner.extend(frame(identity, "success", json!({})));
        }
    }
    let mut segments = vec![segment(
        line(json!({"type":"attached","processId":process_id,
        "stdoutOffset":stdout,"stderrOffset":stderr})),
        None,
        Some(state.opened.clone()),
    )];
    let release = (process.connections == 1).then(|| process.release.clone());
    let mut offset = stdout;
    if matches!(state.fault, Fault::Reconnect) && process.connections == 1 {
        let split = prefix.saturating_sub(7);
        let bytes = &inner[..split];
        offset += bytes.len() as u64;
        process.remaining = inner[split..].to_vec();
        process.offset = offset;
        segments.push(segment(line(json!({"type":"stdout","data":std::str::from_utf8(bytes).unwrap(),"nextOffset":offset})), release, None));
        segments.push(segment(
            line(json!({"type":"reconnect","stdoutOffset":offset,"stderrOffset":0})),
            None,
            None,
        ));
        return reply(segments, false);
    }
    if process.connections > 1 {
        inner = std::mem::take(&mut process.remaining);
    }
    let split = if state.finish.is_some() {
        prefix.min(inner.len())
    } else {
        inner.len()
    };
    for (index, bytes) in [&inner[..split], &inner[split..]].into_iter().enumerate() {
        if bytes.is_empty() {
            continue;
        }
        for (chunk_index, chunk) in bytes.chunks(32 * 1024).enumerate() {
            offset += chunk.len() as u64;
            let next = offset + u64::from(matches!(state.fault, Fault::OffsetGap));
            let wait = if chunk_index > 0 {
                None
            } else if index == 0 {
                release.clone()
            } else {
                state.finish.clone()
            };
            segments.push(segment(line(json!({"type":"stdout","data":std::str::from_utf8(chunk).unwrap(),"nextOffset":next})), wait, None));
        }
    }
    process.offset = offset;
    if matches!(state.fault, Fault::HoldAfterPid | Fault::Backpressure) {
        return reply(segments, true);
    }
    let signal = matches!(state.fault, Fault::SupervisorSignal).then_some("SIGTERM");
    segments.push(segment(
        line(json!({"type":"exit","status":"exited","exitCode":0,
        "signal":signal,"stdoutOffset":offset,"stderrOffset":0})),
        (!matches!(state.fault, Fault::MissingSuccess)).then(|| process.ack.clone()),
        None,
    ));
    if matches!(state.fault, Fault::AfterFinal) {
        segments.push(segment(line(json!({"type":"heartbeat"})), None, None));
    }
    reply(segments, false)
}

fn frame(attempt: &str, kind: &str, fields: Value) -> Vec<u8> {
    let mut frame = json!({"version":1,"attempt":attempt,"kind":kind});
    frame
        .as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    line(frame)
}
fn line(value: Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(b'\n');
    bytes
}
fn segment(bytes: Vec<u8>, wait: Option<Arc<Notify>>, sent: Option<Arc<Notify>>) -> Segment {
    Segment { bytes, wait, sent }
}
fn reply(segments: Vec<Segment>, stall: bool) -> Reply {
    Reply {
        status: 200,
        body: Vec::new(),
        headers: Vec::new(),
        stall,
        segments,
    }
}
