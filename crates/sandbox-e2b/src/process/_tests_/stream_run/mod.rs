//! Shared support for incremental process stream tests.

use std::{sync::Arc, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD};
use bytes::Bytes;
use futures_util::{StreamExt, stream};

use crate::{ConnectProcessTransport, ProcessConnection, StreamProcessCommand};

use crate::process::http::{ByteStream, ConnectHttpTransport};

mod behavior_tests;
mod drop_tests;
mod timeout_tests;

pub(super) fn transport(mock: unimock::Unimock) -> ConnectProcessTransport {
    let http: Arc<dyn ConnectHttpTransport> = Arc::new(mock);
    ConnectProcessTransport {
        http,
        backend_id: "configured-e2b".to_owned(),
    }
}

pub(super) fn connection() -> ProcessConnection {
    ProcessConnection::new(
        "sandbox".to_owned(),
        "e2b.app".to_owned(),
        "access-token".to_owned(),
    )
}

pub(super) fn command(
    stdout_limit: usize,
    stderr_limit: usize,
    deadline: Duration,
    idle_timeout: Duration,
) -> StreamProcessCommand {
    StreamProcessCommand {
        command: "bowser".to_owned(),
        args: vec!["capture".to_owned()],
        stdout_limit,
        stderr_limit,
        deadline,
        idle_timeout,
    }
}

pub(super) fn event_frame(json: &str) -> Vec<u8> {
    super::super::framing::encode_frame(json.as_bytes()).expect("test event frame")
}

pub(super) fn data_frame(channel: &str, value: &str) -> Vec<u8> {
    event_frame(&format!(
        r#"{{"event":{{"data":{{"{channel}":"{}"}}}}}}"#,
        STANDARD.encode(value)
    ))
}

pub(super) fn success_trailer() -> Vec<u8> {
    let mut frame = event_frame("{}");
    frame[0] = 2;
    frame
}

pub(super) fn byte_stream(fragments: Vec<Vec<u8>>) -> ByteStream {
    Box::pin(stream::iter(
        fragments
            .into_iter()
            .map(|fragment| Ok(Bytes::from(fragment))),
    ))
}

pub(super) fn start_then_pending(pid: u32) -> ByteStream {
    let start = stream::once(async move {
        Ok(Bytes::from(event_frame(&format!(
            r#"{{"event":{{"start":{{"pid":{pid}}}}}}}"#
        ))))
    });
    Box::pin(start.chain(stream::pending()))
}

pub(super) fn start_then_periodic(pid: u32, frame: Vec<u8>, every: Duration) -> ByteStream {
    let start = stream::once(async move {
        Ok(Bytes::from(event_frame(&format!(
            r#"{{"event":{{"start":{{"pid":{pid}}}}}}}"#
        ))))
    });
    let recurring = stream::unfold(frame, move |frame| async move {
        tokio::time::sleep(every).await;
        Some((Ok(Bytes::from(frame.clone())), frame))
    });
    Box::pin(start.chain(recurring))
}
