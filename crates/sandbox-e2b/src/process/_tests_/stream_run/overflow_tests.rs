//! Overflow delivery must not wait for a paused consumer to drain its queue.

use std::{sync::Arc, time::Duration};

use futures_util::StreamExt;
use sandbox_interface::{ProcessStreamEvent, ProcessStreamOutcome};
use tokio::sync::Notify;
use unimock::{MockFn, Unimock, matching};

use crate::ProcessTransport;
use crate::process::http::{stream_with_timeout as stream_call, unary as unary_call};

use super::support::{byte_stream, command, connection, data_frame, event_frame, transport};

#[tokio::test(start_paused = true)]
async fn overflow_prefix_and_outcome_do_not_block_cleanup_on_a_full_queue() {
    for channel in ["stdout", "stderr"] {
        let killed = Arc::new(Notify::new());
        let mut fragments = vec![event_frame(r#"{"event":{"start":{"pid":53}}}"#)];
        fragments.extend((0..15).map(|_| data_frame(channel, "x")));
        fragments.push(data_frame(channel, "abc"));
        let transport = transport(Unimock::new((
            stream_call
                .next_call(matching!(_, "Start", _, _))
                .answers_arc(Arc::new(move |_, _, _, _, _| {
                    Ok(byte_stream(fragments.clone()))
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
                command(17, 17, Duration::from_secs(60), Duration::from_secs(60)),
            )
            .await
            .expect("accepted stream");

        tokio::time::timeout(Duration::from_millis(250), killed.notified())
            .await
            .expect("overflow must trigger cleanup before the consumer resumes or timers expire");
        let events = events.collect::<Vec<_>>().await;
        let (output, outcome) = if channel == "stdout" {
            (
                ProcessStreamEvent::Stdout as fn(Vec<u8>) -> ProcessStreamEvent,
                ProcessStreamOutcome::StdoutOverflow,
            )
        } else {
            (
                ProcessStreamEvent::Stderr as fn(Vec<u8>) -> ProcessStreamEvent,
                ProcessStreamOutcome::StderrOverflow,
            )
        };
        let mut expected = vec![ProcessStreamEvent::Started { pid: 53 }];
        expected.extend((0..15).map(|_| output(b"x".to_vec())));
        expected.push(output(b"ab".to_vec()));
        expected.push(ProcessStreamEvent::Outcome(outcome));
        assert_eq!(events, expected);
    }
}
