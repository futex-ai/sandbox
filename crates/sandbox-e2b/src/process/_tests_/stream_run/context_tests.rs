//! Streaming Start bodies preserve explicit context and absent-context encoding.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::StreamExt;
use sandbox_interface::{ProcessStreamEvent, ProcessStreamOutcome};
use unimock::{MockFn, Unimock, matching};

use crate::ProcessTransport;
use crate::process::http::stream_with_timeout as stream_call;

use super::support::{byte_stream, command, connection, event_frame, success_trailer, transport};

#[tokio::test]
async fn stream_start_forwards_working_directory_and_environment() {
    let body = start_body(
        Some("/workspace/repo".to_owned()),
        BTreeMap::from([("SANDBOX_PROBE".to_owned(), "stream-value".to_owned())]),
    )
    .await;

    assert_eq!(body["process"]["cwd"], "/workspace/repo");
    assert_eq!(
        body["process"]["envs"],
        serde_json::json!({"SANDBOX_PROBE": "stream-value"})
    );
}

#[tokio::test]
async fn stream_start_without_context_keeps_the_existing_body() {
    let body = start_body(None, BTreeMap::new()).await;

    assert!(body["process"].get("cwd").is_none());
    assert_eq!(body["process"]["envs"], serde_json::json!({}));
    assert_eq!(
        body,
        serde_json::json!({
            "process": {"cmd": "bowser", "args": ["capture"], "envs": {}},
            "stdin": false
        })
    );
}

async fn start_body(cwd: Option<String>, envs: BTreeMap<String, String>) -> serde_json::Value {
    let captured = Arc::new(Mutex::new(None));
    let transport = transport(Unimock::new(
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc({
                let captured = captured.clone();
                Arc::new(move |_, _, _, body, _| {
                    *captured.lock().expect("captured body lock") = Some(body);
                    Ok(byte_stream(vec![
                        event_frame(r#"{"event":{"start":{"pid":11}}}"#),
                        event_frame(r#"{"event":{"end":{"exitCode":0,"exited":true}}}"#),
                        success_trailer(),
                    ]))
                })
            }),
    ));
    let mut command = command(64, 64, Duration::from_secs(5), Duration::from_secs(2));
    command.cwd = cwd;
    command.envs = envs;
    let events = transport
        .stream_process(connection(), command)
        .await
        .expect("accepted stream")
        .collect::<Vec<_>>()
        .await;
    assert_eq!(
        events.last(),
        Some(&ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::Completed
        ))
    );
    let captured = captured.lock().expect("captured body lock");
    serde_json::from_slice(captured.as_ref().expect("Start body captured")).expect("Start JSON")
}
