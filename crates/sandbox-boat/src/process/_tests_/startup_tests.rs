//! Startup expiry and cancellation preserve outcomes and preparation cleanup.

use std::time::Duration;

use futures_util::{StreamExt, poll};
use sandbox_interface::{
    BackendReadOnlyExecRequest, BackendRunProcessRequest, Error, ProcessStreamEvent,
    ProcessStreamOutcome, SandboxProcessOutput,
};

use crate::runtime::configured::BoatRuntime;

use super::{
    gated_api::Point,
    startup_support::{Work, fixture},
    timing_support::request,
};

#[tokio::test(start_paused = true)]
async fn preparation_failure_at_absolute_deadline_returns_one_deadline_outcome() {
    for (point, fail) in [
        (Point::Bootstrap, true),
        (Point::Upload, true),
        (Point::Bootstrap, false),
        (Point::Upload, false),
    ] {
        let fixture = fixture(point, fail, Work::Unopened);
        let preparing = fixture.runtime.stream_process(request(10, 1));
        tokio::pin!(preparing);
        assert!(poll!(preparing.as_mut()).is_pending());
        fixture.gate.entered.notified().await;
        tokio::time::advance(Duration::from_secs(10)).await;
        fixture.gate.resume.notify_one();
        let events = match preparing.await {
            Ok(stream) => Some(stream.collect::<Vec<_>>().await),
            Err(Error::BackendUnavailable { .. }) => None,
            Err(error) => panic!("unexpected preparation failure: {error:?}"),
        };
        assert_eq!(
            events,
            Some(vec![ProcessStreamEvent::Outcome(
                ProcessStreamOutcome::DeadlineExpired
            )])
        );
    }
}

#[tokio::test(start_paused = true)]
async fn preparation_expiry_keeps_collected_and_stateless_contracts() {
    let collected_fixture = fixture(Point::Bootstrap, true, Work::Unopened);
    let request = request(10, 1);
    let collected = collected_fixture
        .runtime
        .run_process(BackendRunProcessRequest {
            sandbox_provider_ref: request.sandbox_provider_ref.clone(),
            command: request.command.clone(),
            args: request.args.clone(),
            cwd: request.cwd.clone(),
            envs: request.envs.clone(),
            stdout_limit: request.stdout_limit,
            stderr_limit: request.stderr_limit,
            deadline: request.deadline,
        });
    tokio::pin!(collected);
    assert!(poll!(collected.as_mut()).is_pending());
    collected_fixture.gate.entered.notified().await;
    tokio::time::advance(Duration::from_secs(10)).await;
    collected_fixture.gate.resume.notify_one();
    let output = match collected.await {
        Ok(output) => Some(output),
        Err(Error::BackendUnavailable { .. }) => None,
        Err(error) => panic!("unexpected collected failure: {error:?}"),
    };
    assert_eq!(output, Some(SandboxProcessOutput::default()));

    let stateless_fixture = fixture(Point::Bootstrap, true, Work::Unopened);
    let stateless = stateless_fixture
        .runtime
        .read_only_exec(BackendReadOnlyExecRequest {
            sandbox_provider_ref: request.sandbox_provider_ref,
            executable: request.command,
            args: request.args,
            cwd: "/workspace".to_owned(),
            output_limit: 4096,
            timeout: request.deadline,
        });
    tokio::pin!(stateless);
    assert!(poll!(stateless.as_mut()).is_pending());
    stateless_fixture.gate.entered.notified().await;
    tokio::time::advance(Duration::from_secs(10)).await;
    stateless_fixture.gate.resume.notify_one();
    assert!(matches!(
        stateless.await,
        Err(Error::BackendUnavailable { .. })
    ));
}

#[tokio::test(start_paused = true)]
async fn drop_after_first_poll_before_reader_runs_removes_prepared_state() {
    let fixture = fixture(Point::Frame, false, Work::Unopened);
    let mut events = fixture
        .runtime
        .stream_process(request(10, 1))
        .await
        .unwrap();
    assert!(poll!(events.next()).is_pending());
    drop(events);
    let cleaned = tokio::time::timeout(Duration::from_secs(10), fixture.cleaned.notified()).await;
    assert_eq!(
        cleaned,
        Ok(()),
        "prepared state must have a cleanup owner after handoff"
    );
    let state = fixture.state.lock().unwrap();
    assert!(!state.prepared);
    assert_eq!(
        state
            .actions
            .iter()
            .filter(|action| *action == "clean")
            .count(),
        1
    );
    assert!(
        !state
            .actions
            .iter()
            .any(|action| matches!(action.as_str(), "release" | "terminate"))
    );
    assert_eq!(state.stops, 0);
}

#[tokio::test(start_paused = true)]
async fn drop_before_release_cleans_state_but_dispatched_release_retains_unknown_pid_state() {
    for point in [Point::Frame, Point::Release] {
        let fixture = fixture(point, false, Work::Cancel);
        let mut events = fixture
            .runtime
            .stream_process(request(10, 1))
            .await
            .unwrap();
        let pending = tokio::spawn(async move { events.next().await });
        fixture.gate.entered.notified().await;
        pending.abort();
        let _ = pending.await;
        let settled = if point == Point::Frame {
            fixture.gate.resume.notify_one();
            tokio::time::timeout(Duration::from_secs(10), fixture.cleaned.notified()).await
        } else {
            tokio::task::yield_now().await;
            tokio::time::advance(Duration::from_secs(3)).await;
            tokio::time::timeout(Duration::from_nanos(1), fixture.detached.notified()).await
        };
        assert_eq!(settled, Ok(()), "cancellation must resolve its owned state");
        let state = fixture.state.lock().unwrap();
        let released = point == Point::Release;
        assert_eq!(state.prepared, released);
        assert_eq!(
            state
                .actions
                .iter()
                .filter(|action| *action == "terminate")
                .count(),
            usize::from(!released)
        );
        assert_eq!(
            state
                .actions
                .iter()
                .filter(|action| *action == "clean")
                .count(),
            usize::from(!released)
        );
        assert_eq!(state.actions.contains(&"release".to_owned()), released);
        assert_eq!(state.stops, 0);
    }
}

#[tokio::test(start_paused = true)]
async fn release_expiry_before_child_exit_preserves_deadline_and_idle_outcomes() {
    for (deadline, idle, expected) in [
        (2, 2, ProcessStreamOutcome::DeadlineExpired),
        (10, 1, ProcessStreamOutcome::IdleTimeout),
    ] {
        let fixture = fixture(Point::Release, false, Work::TimedOut);
        let events = fixture
            .runtime
            .stream_process(request(deadline, idle))
            .await
            .unwrap();
        let pending = tokio::spawn(async move { events.collect::<Vec<_>>().await });
        fixture.gate.entered.notified().await;
        tokio::time::advance(Duration::from_secs(deadline.min(idle))).await;
        assert_eq!(
            pending.await.unwrap(),
            vec![ProcessStreamEvent::Outcome(expected)]
        );
    }
}
