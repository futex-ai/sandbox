//! Consumer-drop process cleanup regression.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use bytes::Bytes;
use futures_util::{StreamExt, stream};
use sandbox_interface::{ProcessEventStream, ProcessStreamEvent};
use tokio::sync::Notify;
use unimock::{MockFn, Unimock, matching};

use crate::ProcessTransport;
use crate::process::http::{stream_with_timeout as stream_call, unary as unary_call};

use super::{
    gated_open,
    support::{command, connection, event_frame, start_then_pending, transport},
};

#[tokio::test]
async fn dropping_consumer_stream_kills_an_observed_process() {
    let killed = Arc::new(Notify::new());
    let transport = transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers(&|_, _, _, _, _| Ok(start_then_pending(37))),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .answers_arc({
                let killed = killed.clone();
                Arc::new(move |_, _, _, request: Vec<u8>, _| {
                    assert_eq!(
                        request,
                        br#"{"process":{"pid":37},"signal":"SIGNAL_SIGKILL"}"#
                    );
                    killed.notify_one();
                    Ok(Vec::new())
                })
            }),
    )));

    {
        let mut stream = transport
            .stream_process(
                connection(),
                command(64, 64, Duration::from_secs(5), Duration::from_secs(2)),
            )
            .await
            .expect("stream should start");
        assert_eq!(
            stream.next().await,
            Some(ProcessStreamEvent::Started { pid: 37 })
        );
    }

    tokio::time::timeout(Duration::from_secs(1), killed.notified())
        .await
        .expect("dropping the consumer must trigger process cleanup");
}

#[tokio::test]
async fn dropping_before_open_is_polled_makes_no_provider_call() {
    let transport = transport(Unimock::new(()));
    let stream = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_secs(5), Duration::from_secs(4)),
        )
        .await
        .expect("accepted stream");
    drop(stream);
    tokio::task::yield_now().await;
}

#[tokio::test]
async fn dropping_with_a_decoded_but_unprocessed_start_kills_its_pid() {
    let consumer = Arc::new(Mutex::new(None::<ProcessEventStream>));
    let killed = Arc::new(Notify::new());
    let transport = transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc({
                let consumer = consumer.clone();
                Arc::new(move |_, _, _, _, _| {
                    let consumer = consumer.clone();
                    Ok(Box::pin(
                        stream::once(async move {
                            consumer.lock().expect("consumer lock").take();
                            Ok(Bytes::from(event_frame(
                                r#"{"event":{"start":{"pid":53}}}"#,
                            )))
                        })
                        .chain(stream::pending()),
                    ))
                })
            }),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .answers_arc({
                let killed = killed.clone();
                Arc::new(move |_, _, _, request: Vec<u8>, _| {
                    assert_eq!(
                        request,
                        br#"{"process":{"pid":53},"signal":"SIGNAL_SIGKILL"}"#
                    );
                    killed.notify_one();
                    Ok(Vec::new())
                })
            }),
    )));
    *consumer.lock().expect("consumer lock") = Some(
        transport
            .stream_process(
                connection(),
                command(64, 64, Duration::from_secs(5), Duration::from_secs(4)),
            )
            .await
            .expect("accepted stream"),
    );
    tokio::time::timeout(Duration::from_secs(1), killed.notified())
        .await
        .expect("a staged start must still be killed");
}

#[tokio::test]
async fn dropping_during_pending_open_keeps_open_alive_for_start() {
    let entered = Arc::new(Notify::new());
    let resume = Arc::new(Notify::new());
    let killed = Arc::new(Notify::new());
    let transport = gated_open::transport(
        Unimock::new((
            stream_call
                .next_call(matching!(_, "Start", _, _))
                .answers(&|_, _, _, _, _| Ok(start_then_pending(59))),
            unary_call
                .next_call(matching!(_, "SendSignal", _, false))
                .answers_arc({
                    let killed = killed.clone();
                    Arc::new(move |_, _, _, request: Vec<u8>, _| {
                        assert_eq!(
                            request,
                            br#"{"process":{"pid":59},"signal":"SIGNAL_SIGKILL"}"#
                        );
                        killed.notify_one();
                        Ok(Vec::new())
                    })
                }),
        )),
        entered.clone(),
        resume.clone(),
    );
    let events = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_secs(5), Duration::from_secs(4)),
        )
        .await
        .expect("accepted stream");
    entered.notified().await;
    drop(events);
    resume.notify_one();
    tokio::time::timeout(Duration::from_secs(1), killed.notified())
        .await
        .expect("pending open must stay alive long enough to learn PID");
}

#[tokio::test]
async fn dropping_after_open_before_start_kills_late_start() {
    let polled = Arc::new(Notify::new());
    let resume = Arc::new(Notify::new());
    let killed = Arc::new(Notify::new());
    let transport = transport(Unimock::new((
        stream_call
            .next_call(matching!(_, "Start", _, _))
            .answers_arc({
                let polled = polled.clone();
                let resume = resume.clone();
                Arc::new(move |_, _, _, _, _| {
                    let polled = polled.clone();
                    let resume = resume.clone();
                    Ok(Box::pin(
                        stream::once(async move {
                            polled.notify_one();
                            resume.notified().await;
                            Ok(Bytes::from(event_frame(
                                r#"{"event":{"start":{"pid":61}}}"#,
                            )))
                        })
                        .chain(stream::pending()),
                    ))
                })
            }),
        unary_call
            .next_call(matching!(_, "SendSignal", _, false))
            .answers_arc({
                let killed = killed.clone();
                Arc::new(move |_, _, _, request: Vec<u8>, _| {
                    assert_eq!(
                        request,
                        br#"{"process":{"pid":61},"signal":"SIGNAL_SIGKILL"}"#
                    );
                    killed.notify_one();
                    Ok(Vec::new())
                })
            }),
    )));
    let events = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_secs(5), Duration::from_secs(4)),
        )
        .await
        .expect("accepted stream");
    polled.notified().await;
    drop(events);
    resume.notify_one();
    tokio::time::timeout(Duration::from_secs(1), killed.notified())
        .await
        .expect("late start must be killed");
}
