//! Decoded process ends protect cleanup even before staging or delivery.

use std::{
    sync::{Arc, Mutex},
    task::Poll,
    time::Duration,
};

use bytes::Bytes;
use futures_util::{StreamExt, stream};
use sandbox_interface::{ProcessEventStream, ProcessStreamEvent, ProcessStreamOutcome};
use tokio::sync::Notify;
use unimock::{MockFn, Unimock, matching};

use crate::ProcessTransport;
use crate::process::http::{stream_with_timeout as stream_call, unary as unary_call};

use super::support::{NotifyOnDrop, command, connection, data_frame, event_frame, transport};

#[tokio::test]
async fn coalesced_start_and_end_after_drop_do_not_send_signal() {
    assert_dropped_chunk_signals(
        [
            event_frame(r#"{"event":{"start":{"pid":83}}}"#),
            event_frame(r#"{"event":{"end":{"exitCode":0,"exited":true}}}"#),
        ]
        .concat(),
        Vec::new(),
    )
    .await;
}

#[tokio::test]
async fn failure_before_coalesced_end_still_kills_started_process() {
    assert_dropped_chunk_signals(
        [
            event_frame(r#"{"event":{"start":{"pid":83}}}"#),
            event_frame(r#"{"event":{"unexpected":{}}}"#),
            event_frame(r#"{"event":{"end":{"exitCode":0,"exited":true}}}"#),
        ]
        .concat(),
        vec![br#"{"process":{"pid":83},"signal":"SIGNAL_SIGKILL"}"#.to_vec()],
    )
    .await;
}

async fn assert_dropped_chunk_signals(chunk: Vec<u8>, expected: Vec<Vec<u8>>) {
    let consumer = Arc::new(Mutex::new(None::<ProcessEventStream>));
    let released = Arc::new(Notify::new());
    let requests = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let transport = transport(
        Unimock::new((
            stream_call
                .next_call(matching!(_, "Start", _, _))
                .answers_arc({
                    let consumer = consumer.clone();
                    let released = released.clone();
                    Arc::new(move |_, _, _, _, _| {
                        let consumer = consumer.clone();
                        let guard = NotifyOnDrop(released.clone());
                        let chunk = chunk.clone();
                        Ok(Box::pin(
                            stream::once(async move {
                                consumer.lock().expect("consumer lock").take();
                                Ok(Bytes::from(chunk))
                            })
                            .chain(stream::poll_fn(move |_| {
                                let _ = &guard;
                                Poll::Pending
                            })),
                        ))
                    })
                }),
            unary_call
                .each_call(matching!(_, "SendSignal", _, false))
                .answers_arc({
                    let requests = requests.clone();
                    Arc::new(move |_, _, _, request: Vec<u8>, _| {
                        requests.lock().expect("requests lock").push(request);
                        Ok(Vec::new())
                    })
                }),
        ))
        .no_verify_in_drop(),
    );
    *consumer.lock().expect("consumer lock") = Some(
        transport
            .stream_process(
                connection(),
                command(64, 64, Duration::from_secs(5), Duration::from_secs(4)),
            )
            .await
            .expect("accepted stream"),
    );
    tokio::time::timeout(Duration::from_secs(1), released.notified())
        .await
        .expect("reader must release the provider stream after drop");
    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert_eq!(*requests.lock().expect("requests lock"), expected);
}

#[tokio::test]
async fn coalesced_end_after_overflow_prevents_kill() {
    let released = Arc::new(Notify::new());
    let requests = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let chunk = [
        event_frame(r#"{"event":{"start":{"pid":89}}}"#),
        data_frame("stdout", "abc"),
        event_frame(r#"{"event":{"end":{"exitCode":0,"exited":true}}}"#),
    ]
    .concat();
    let transport = transport(
        Unimock::new((
            stream_call
                .next_call(matching!(_, "Start", _, _))
                .answers_arc({
                    let released = released.clone();
                    Arc::new(move |_, _, _, _, _| {
                        let guard = NotifyOnDrop(released.clone());
                        let chunk = chunk.clone();
                        Ok(Box::pin(
                            stream::once(async move { Ok(Bytes::from(chunk)) }).chain(
                                stream::poll_fn(move |_| {
                                    let _ = &guard;
                                    Poll::Pending
                                }),
                            ),
                        ))
                    })
                }),
            unary_call
                .each_call(matching!(_, "SendSignal", _, false))
                .answers_arc({
                    let requests = requests.clone();
                    Arc::new(move |_, _, _, request: Vec<u8>, _| {
                        requests.lock().expect("requests lock").push(request);
                        Ok(Vec::new())
                    })
                }),
        ))
        .no_verify_in_drop(),
    );
    let events = transport
        .stream_process(
            connection(),
            command(2, 64, Duration::from_secs(5), Duration::from_secs(4)),
        )
        .await
        .expect("accepted stream");
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), events.collect::<Vec<_>>())
            .await
            .expect("stream finishes"),
        [
            ProcessStreamEvent::Started { pid: 89 },
            ProcessStreamEvent::Stdout(b"ab".to_vec()),
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::StdoutOverflow),
        ]
    );
    tokio::time::timeout(Duration::from_secs(1), released.notified())
        .await
        .expect("provider stream released after overflow");
    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(
        requests.lock().expect("requests lock").is_empty(),
        "ended process must not be killed"
    );
}
