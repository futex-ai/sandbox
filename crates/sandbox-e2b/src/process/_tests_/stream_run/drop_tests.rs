//! Consumer-drop process cleanup regression.

use std::{sync::Arc, time::Duration};

use futures_util::StreamExt;
use sandbox_interface::ProcessStreamEvent;
use tokio::sync::Notify;
use unimock::{MockFn, Unimock, matching};

use crate::ProcessTransport;
use crate::process::http::{stream_with_timeout as stream_call, unary as unary_call};

use super::support::{command, connection, start_then_pending, transport};

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
