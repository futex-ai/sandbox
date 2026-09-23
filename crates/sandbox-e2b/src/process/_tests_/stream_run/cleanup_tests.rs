//! Terminal outcome ordering relative to best-effort process cleanup.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use futures_util::StreamExt;
use sandbox_interface::{ProcessStreamEvent, ProcessStreamOutcome};
use tokio::sync::Notify;
use unimock::{MockFn, Unimock, matching};

use crate::error::Result;
use crate::{ConnectProcessTransport, ProcessConnection, ProcessTransport};

use crate::process::http::{
    ByteStream, ConnectHttpTransport, stream_with_timeout as stream_call, unary as unary_call,
};

use super::support::{
    byte_stream, command, connection, data_frame, event_frame, start_then_pending, transport,
};

struct StalledKillHttp {
    kill_started: Arc<Notify>,
    kill_release: Arc<Notify>,
    kill_finished: Arc<Notify>,
}

#[async_trait]
impl ConnectHttpTransport for StalledKillHttp {
    async fn stream(
        &self,
        _connection: ProcessConnection,
        _method: String,
        _request_json: Vec<u8>,
    ) -> Result<ByteStream> {
        unreachable!("streaming process uses its deadline-derived request timeout")
    }

    async fn stream_with_timeout(
        &self,
        _connection: ProcessConnection,
        method: String,
        _request_json: Vec<u8>,
        _request_timeout: Duration,
    ) -> Result<ByteStream> {
        assert_eq!(method, "Start");
        Ok(start_then_pending(43))
    }

    async fn unary(
        &self,
        _connection: ProcessConnection,
        method: String,
        _request_json: Vec<u8>,
        ambiguous: bool,
    ) -> Result<Vec<u8>> {
        assert_eq!(method, "SendSignal");
        assert!(!ambiguous);
        self.kill_started.notify_one();
        self.kill_release.notified().await;
        self.kill_finished.notify_one();
        Ok(Vec::new())
    }

    async fn download(
        &self,
        _connection: ProcessConnection,
        _path: String,
        _offset: u64,
        _max_bytes: usize,
    ) -> Result<Vec<u8>> {
        unreachable!("file download is outside this test")
    }

    async fn upload(
        &self,
        _connection: ProcessConnection,
        _path: String,
        _bytes: Vec<u8>,
    ) -> Result<()> {
        unreachable!("file upload is outside this test")
    }
}

#[tokio::test]
async fn deadline_outcome_and_eof_precede_stalled_kill_cleanup() {
    let kill_started = Arc::new(Notify::new());
    let kill_release = Arc::new(Notify::new());
    let kill_finished = Arc::new(Notify::new());
    let transport = ConnectProcessTransport {
        http: Arc::new(StalledKillHttp {
            kill_started: kill_started.clone(),
            kill_release: kill_release.clone(),
            kill_finished: kill_finished.clone(),
        }),
        backend_id: "configured-e2b".to_owned(),
    };
    let mut stream = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_millis(30), Duration::from_millis(30)),
        )
        .await
        .expect("stream should start");

    assert_eq!(
        stream.next().await,
        Some(ProcessStreamEvent::Started { pid: 43 })
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_millis(250), stream.next())
            .await
            .expect("deadline outcome must not wait for cleanup"),
        Some(ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::DeadlineExpired
        ))
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_millis(50), stream.next())
            .await
            .expect("stream must close before cleanup finishes"),
        None
    );
    tokio::time::timeout(Duration::from_millis(50), kill_started.notified())
        .await
        .expect("best-effort cleanup should still begin");
    kill_release.notify_one();
    tokio::time::timeout(Duration::from_millis(50), kill_finished.notified())
        .await
        .expect("best-effort cleanup should finish after release");
}

#[tokio::test]
async fn full_event_queue_does_not_block_cleanup_or_terminal_outcome() {
    let kill_started = Arc::new(Notify::new());
    let mut fragments = vec![event_frame(r#"{"event":{"start":{"pid":47}}}"#)];
    fragments.extend((0..32).map(|_| data_frame("stdout", "x")));
    let transport = transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc(Arc::new(move |_, _, _, _, _| {
                Ok(byte_stream(fragments.clone()))
            })),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .answers_arc({
                let kill_started = kill_started.clone();
                Arc::new(move |_, _, _, _, _| {
                    kill_started.notify_one();
                    Ok(Vec::new())
                })
            }),
    )));
    let stream = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_millis(30), Duration::from_millis(30)),
        )
        .await
        .expect("stream should start");

    tokio::time::timeout(Duration::from_millis(250), kill_started.notified())
        .await
        .expect("a full event queue must not block cleanup");
    let events = tokio::time::timeout(Duration::from_millis(250), stream.collect::<Vec<_>>())
        .await
        .expect("queued data and terminal outcome should remain drainable");

    assert_eq!(
        events.first(),
        Some(&ProcessStreamEvent::Started { pid: 47 })
    );
    assert!(
        events[1..events.len() - 1]
            .iter()
            .all(|event| matches!(event, ProcessStreamEvent::Stdout(bytes) if bytes == b"x"))
    );
    assert_eq!(
        events.last(),
        Some(&ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::DeadlineExpired
        ))
    );
}
