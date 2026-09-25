//! A consumer drop must allow PID discovery beyond the normal idle timeout.

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use futures_util::{StreamExt, stream};
use tokio::sync::Notify;
use unimock::{MockFn, Unimock, matching};

use crate::ProcessTransport;
use crate::process::http::{stream_with_timeout as stream_call, unary as unary_call};

use super::{
    gated_open,
    support::{command, connection, event_frame, start_then_pending, transport},
};

#[tokio::test(start_paused = true)]
async fn dropped_pending_open_kills_start_after_idle_expiry_within_grace() {
    let entered = Arc::new(Notify::new());
    let resume = Arc::new(Notify::new());
    let killed = Arc::new(Notify::new());
    let transport = gated_open::transport(
        Unimock::new((
            stream_call
                .next_call(matching!(_, "Start", _, _))
                .answers(&|_, _, _, _, _| Ok(start_then_pending(71))),
            unary_call
                .next_call(matching!(_, "SendSignal", _, false))
                .answers_arc({
                    let killed = killed.clone();
                    Arc::new(move |_, _, _, request: Vec<u8>, _| {
                        assert_eq!(
                            request,
                            br#"{"process":{"pid":71},"signal":"SIGNAL_SIGKILL"}"#
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
            command(64, 64, Duration::from_secs(5), Duration::from_secs(1)),
        )
        .await
        .expect("accepted stream");
    entered.notified().await;
    drop(events);
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(2)).await;
    resume.notify_one();
    tokio::time::timeout(Duration::from_millis(10), killed.notified())
        .await
        .expect("start inside drop grace must be killed after normal idle expiry");
}

#[tokio::test(start_paused = true)]
async fn dropped_reader_kills_start_after_idle_expiry_within_grace() {
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
                                r#"{"event":{"start":{"pid":73}}}"#,
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
                        br#"{"process":{"pid":73},"signal":"SIGNAL_SIGKILL"}"#
                    );
                    killed.notify_one();
                    Ok(Vec::new())
                })
            }),
    )));
    let events = transport
        .stream_process(
            connection(),
            command(64, 64, Duration::from_secs(5), Duration::from_secs(1)),
        )
        .await
        .expect("accepted stream");
    polled.notified().await;
    drop(events);
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(2)).await;
    resume.notify_one();
    tokio::time::timeout(Duration::from_millis(10), killed.notified())
        .await
        .expect("reader must keep looking for start beyond normal idle expiry");
}
