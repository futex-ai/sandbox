//! Drop-grace expiry and decoded process-end cleanup regressions.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::Poll,
    time::Duration,
};

use futures_util::{StreamExt, stream};
use tokio::sync::Notify;
use unimock::{MockFn, Unimock, matching};

use crate::ProcessTransport;
use crate::process::http::{stream_with_timeout as stream_call, unary as unary_call};

use super::support::{
    NotifyOnDrop, byte_stream, command, connection, data_frame, event_frame, transport,
};

#[tokio::test(start_paused = true)]
async fn missing_start_after_drop_releases_provider_within_grace() {
    assert_no_pid_release_by(
        Duration::from_secs(5),
        Duration::from_secs(5),
        Duration::from_secs(3),
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn drop_grace_never_outlives_absolute_deadline() {
    assert_no_pid_release_by(
        Duration::from_secs(1),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .await;
}

async fn assert_no_pid_release_by(deadline: Duration, idle_timeout: Duration, wait: Duration) {
    let polled = Arc::new(Notify::new());
    let released = Arc::new(Notify::new());
    let signals = Arc::new(AtomicUsize::new(0));
    let transport = transport(
        Unimock::new((
            stream_call
                .next_call(matching!(_, "Start", _, _))
                .answers_arc({
                    let polled = polled.clone();
                    let released = released.clone();
                    Arc::new(move |_, _, _, _, _| {
                        let polled = polled.clone();
                        let guard = NotifyOnDrop(released.clone());
                        Ok(Box::pin(stream::poll_fn(move |_| {
                            let _ = &guard;
                            polled.notify_one();
                            Poll::Pending
                        })))
                    })
                }),
            unary_call
                .each_call(matching!(_, "SendSignal", _, false))
                .answers_arc({
                    let signals = signals.clone();
                    Arc::new(move |_, _, _, _, _| {
                        signals.fetch_add(1, Ordering::SeqCst);
                        Ok(Vec::new())
                    })
                }),
        ))
        .no_verify_in_drop(),
    );
    let events = transport
        .stream_process(connection(), command(64, 64, deadline, idle_timeout))
        .await
        .expect("accepted stream");
    polled.notified().await;
    drop(events);
    tokio::task::yield_now().await;
    tokio::time::advance(wait).await;
    tokio::time::timeout(Duration::from_millis(1), released.notified())
        .await
        .expect("provider stream must be released at the grace limit");
    assert_eq!(signals.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn decoded_end_before_drop_prevents_kill() {
    let polled_after_end = Arc::new(Notify::new());
    let released = Arc::new(Notify::new());
    let signals = Arc::new(AtomicUsize::new(0));
    let mut frames = vec![event_frame(r#"{"event":{"start":{"pid":67}}}"#)];
    frames.extend((0..18).map(|_| data_frame("stdout", "x")));
    frames.push(event_frame(
        r#"{"event":{"end":{"exitCode":0,"exited":true}}}"#,
    ));
    let transport = transport(
        Unimock::new((
            stream_call
                .next_call(matching!(_, "Start", _, _))
                .answers_arc({
                    let polled_after_end = polled_after_end.clone();
                    let released = released.clone();
                    Arc::new(move |_, _, _, _, _| {
                        let guard = NotifyOnDrop(released.clone());
                        let polled_after_end = polled_after_end.clone();
                        Ok(Box::pin(byte_stream(vec![frames.concat()]).chain(
                            stream::poll_fn(move |_| {
                                let _ = &guard;
                                polled_after_end.notify_one();
                                Poll::Pending
                            }),
                        )))
                    })
                }),
            unary_call
                .each_call(matching!(_, "SendSignal", _, false))
                .answers_arc({
                    let signals = signals.clone();
                    Arc::new(move |_, _, _, _, _| {
                        signals.fetch_add(1, Ordering::SeqCst);
                        Ok(Vec::new())
                    })
                }),
        ))
        .no_verify_in_drop(),
    );
    let events = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_secs(5), Duration::from_secs(4)),
        )
        .await
        .expect("accepted stream");
    polled_after_end.notified().await;
    drop(events);
    tokio::time::timeout(Duration::from_secs(1), released.notified())
        .await
        .expect("provider reader must finish after drop");
    tokio::task::yield_now().await;
    assert_eq!(signals.load(Ordering::SeqCst), 0);
}
