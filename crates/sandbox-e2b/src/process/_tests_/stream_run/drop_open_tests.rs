//! Pending open cancellation obeys the shorter of drop grace and absolute budget.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use tokio::sync::Notify;
use unimock::{MockFn, Unimock, matching};

use crate::ProcessTransport;
use crate::process::http::unary as unary_call;

use super::{
    gated_open,
    support::{command, connection},
};

#[tokio::test(start_paused = true)]
async fn pending_open_drops_within_three_seconds_after_consumer_drop() {
    assert_pending_open_released_by(Duration::from_secs(5), Duration::from_secs(3)).await;
}

#[tokio::test(start_paused = true)]
async fn pending_open_drops_at_absolute_deadline_before_grace() {
    assert_pending_open_released_by(Duration::from_secs(1), Duration::from_secs(1)).await;
}

async fn assert_pending_open_released_by(deadline: Duration, wait: Duration) {
    let entered = Arc::new(Notify::new());
    let resume = Arc::new(Notify::new());
    let released = Arc::new(Notify::new());
    let signals = Arc::new(AtomicUsize::new(0));
    let transport = gated_open::transport_with_release(
        Unimock::new(
            unary_call
                .each_call(matching!(_, "SendSignal", _, false))
                .answers_arc({
                    let signals = signals.clone();
                    Arc::new(move |_, _, _, _, _| {
                        signals.fetch_add(1, Ordering::SeqCst);
                        Ok(Vec::new())
                    })
                }),
        )
        .no_verify_in_drop(),
        entered.clone(),
        resume,
        Some(released.clone()),
    );
    let events = transport
        .stream_process(connection(), command(64, 64, deadline, deadline))
        .await
        .expect("accepted stream");
    entered.notified().await;
    drop(events);
    tokio::task::yield_now().await;
    tokio::time::advance(wait).await;
    tokio::time::timeout(Duration::from_millis(1), released.notified())
        .await
        .expect("pending open must be dropped by its bounded grace");
    tokio::task::yield_now().await;
    assert_eq!(signals.load(Ordering::SeqCst), 0);
}
