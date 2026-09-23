//! Buffered provider fragments and slow process stream consumers.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use bytes::Bytes;
use futures_util::{StreamExt, stream};
use sandbox_interface::{ProcessStreamEvent, ProcessStreamOutcome};
use tokio::sync::Notify;
use unimock::{MockFn, Unimock, matching};

use crate::ProcessTransport;
use crate::process::http::{stream_with_timeout as stream_call, unary as unary_call};

use super::super::EVENT_CHANNEL_CAPACITY;
use super::support::{byte_stream, command, connection, data_frame, event_frame, transport};

#[tokio::test(start_paused = true)]
async fn previously_buffered_http_fragments_do_not_extend_idle_time() {
    let killed = Arc::new(Notify::new());
    let observed = Arc::new(AtomicUsize::new(0));
    let mut fragments = vec![event_frame(r#"{"event":{"start":{"pid":61}}}"#)];
    fragments.extend((0..18).map(|_| data_frame("stdout", "x")));
    let transport = transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc({
                let observed = observed.clone();
                Arc::new(move |_, _, _, _, _| {
                    let observed = observed.clone();
                    Ok(Box::pin(
                        byte_stream(fragments.clone())
                            .inspect(move |_| {
                                observed.fetch_add(1, Ordering::SeqCst);
                            })
                            .chain(stream::pending()),
                    ))
                })
            }),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .answers_arc({
                let killed = killed.clone();
                Arc::new(move |_, _, _, _, _| {
                    killed.notify_one();
                    Ok(Vec::new())
                })
            }),
    )));
    let mut events = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_secs(1), Duration::from_millis(20)),
        )
        .await
        .expect("stream should start");

    for _ in 0..128 {
        if observed.load(Ordering::SeqCst) == 19 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(observed.load(Ordering::SeqCst), 19);
    tokio::time::advance(Duration::from_millis(10)).await;
    assert_eq!(
        events.next().await,
        Some(ProcessStreamEvent::Started { pid: 61 })
    );
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(11)).await;
    tokio::time::timeout(Duration::from_millis(1), killed.notified())
        .await
        .expect("buffered fragments must not delay idle cleanup");

    let remaining = events.collect::<Vec<_>>().await;
    assert_eq!(
        remaining.last(),
        Some(&ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::IdleTimeout
        ))
    );
}

#[tokio::test(start_paused = true)]
async fn full_staging_stops_the_process_without_waiting_for_consumer_capacity() {
    let resume = Arc::new(Notify::new());
    let killed = Arc::new(Notify::new());
    let data: Vec<_> = (0..100).map(|_| data_frame("stdout", "x")).collect();
    let transport = transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc({
                let resume = resume.clone();
                Arc::new(move |_, _, _, _, _| {
                    let resume = resume.clone();
                    let start = stream::once(async {
                        Ok(Bytes::from(event_frame(
                            r#"{"event":{"start":{"pid":67}}}"#,
                        )))
                    });
                    let gated = stream::once(async move {
                        resume.notified().await;
                        Ok(Bytes::from(data_frame("stdout", "x")))
                    });
                    Ok(Box::pin(
                        start.chain(gated).chain(byte_stream(data.clone())),
                    ))
                })
            }),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .answers_arc({
                let killed = killed.clone();
                Arc::new(move |_, _, _, _, _| {
                    killed.notify_one();
                    Ok(Vec::new())
                })
            }),
    )));
    let mut events = transport
        .stream_process(
            connection(),
            command(1024, 64, Duration::from_secs(1), Duration::from_secs(1)),
        )
        .await
        .expect("stream should start");

    assert_eq!(
        events.next().await,
        Some(ProcessStreamEvent::Started { pid: 67 })
    );
    resume.notify_one();
    tokio::time::timeout(Duration::from_millis(5), killed.notified())
        .await
        .expect("saturation must trigger cleanup before the consumer drains data");

    let remaining = events.collect::<Vec<_>>().await;
    assert!(remaining.len() <= EVENT_CHANNEL_CAPACITY + 1);
    assert!(
        remaining[..remaining.len() - 1]
            .iter()
            .all(|event| matches!(event, ProcessStreamEvent::Stdout(bytes) if bytes == b"x"))
    );
    assert_eq!(
        remaining.last(),
        Some(&ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::ConsumerBackpressure
        ))
    );
}

#[tokio::test(start_paused = true)]
async fn fresh_provider_output_refreshes_idle_time_even_when_delivery_is_blocked() {
    let killed = Arc::new(Notify::new());
    let mut queued = vec![event_frame(r#"{"event":{"start":{"pid":71}}}"#)];
    queued.extend((0..18).map(|_| data_frame("stdout", "x")));
    let transport = transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc(Arc::new(move |_, _, _, _, _| {
                let delayed = stream::once(async {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    Ok(Bytes::from(data_frame("stdout", "y")))
                });
                Ok(Box::pin(
                    byte_stream(queued.clone())
                        .chain(delayed)
                        .chain(stream::pending()),
                ))
            })),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .answers_arc({
                let killed = killed.clone();
                Arc::new(move |_, _, _, _, _| {
                    killed.notify_one();
                    Ok(Vec::new())
                })
            }),
    )));
    let events = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_secs(1), Duration::from_millis(20)),
        )
        .await
        .expect("stream should start");

    tokio::time::sleep(Duration::from_millis(1)).await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    tokio::time::advance(Duration::from_millis(11)).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(1), killed.notified())
            .await
            .is_err(),
        "fresh provider output must postpone the original idle deadline"
    );
    tokio::time::advance(Duration::from_millis(20)).await;
    tokio::time::timeout(Duration::from_millis(1), killed.notified())
        .await
        .expect("idle cleanup must follow the most recent output");
    assert_eq!(
        events.collect::<Vec<_>>().await.last(),
        Some(&ProcessStreamEvent::Outcome(
            ProcessStreamOutcome::IdleTimeout
        ))
    );
}
