//! Process public APIs use real HTTPS serialization and exact framed bytes.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use futures_util::StreamExt;
use sandbox_interface::{
    BackendReadOnlyExecRequest, BackendRunProcessRequest, BackendStreamProcessRequest,
    ProcessStreamEvent, ProcessStreamOutcome, ProviderRef, SandboxProcesses, conformance,
};
use tokio::sync::Notify;

use super::support::fixture::Fixture;

pub(super) fn request() -> BackendStreamProcessRequest {
    BackendStreamProcessRequest {
        sandbox_provider_ref: ProviderRef::new("bx_abcdefgh"),
        command: "/usr/bin/python3".to_owned(),
        args: vec!["a b'$(secret)\n".to_owned()],
        cwd: Some("/workspace".to_owned()),
        envs: BTreeMap::from([("EXACT".to_owned(), "secret-value".to_owned())]),
        stdout_limit: 4096,
        stderr_limit: 4096,
        deadline: Duration::from_secs(60),
        idle_timeout: Duration::from_secs(30),
    }
}

#[tokio::test]
async fn process_and_file_conformance_run_through_verified_runtime_https() {
    let fixture = Fixture::start().await;
    assert!(matches!(
        conformance::exercise_processes(&fixture.backend, ProviderRef::new("bx_abcdefgh")).await,
        Ok(())
    ));
    assert!(matches!(
        conformance::exercise_files(&fixture.backend, ProviderRef::new("bx_abcdefgh")).await,
        Ok(())
    ));
    let requests = fixture.captured();
    assert!(requests.iter().all(|r| !r.path.contains("history")
        && !r.path.contains("resume")
        && !r.path.contains("prompts")));
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.body["detached"] == true)
            .count(),
        2
    );
    assert_eq!(fixture.state.lock().unwrap().runtime.staging.len(), 0);
}

#[tokio::test]
async fn streaming_delivers_exact_binary_channels_before_exit_and_keeps_child_pid() {
    let fixture = Fixture::start().await;
    let finish = Arc::new(Notify::new());
    {
        let mut state = fixture.state.lock().unwrap();
        state.runtime.stdout = b"\0\xffstdout".to_vec();
        state.runtime.stderr = b"\xfestderr".to_vec();
        state.runtime.finish = Some(finish.clone());
        state.runtime.code = 7;
    }
    let mut stream = fixture.backend.stream_process(request()).await.unwrap();
    assert_eq!(
        stream.next().await,
        Some(ProcessStreamEvent::Started { pid: 2001 })
    );
    assert_eq!(
        stream.next().await,
        Some(ProcessStreamEvent::Stdout(b"\0\xffstdout".to_vec()))
    );
    assert_eq!(
        stream.next().await,
        Some(ProcessStreamEvent::Stderr(b"\xfestderr".to_vec()))
    );
    finish.notify_one();
    assert_eq!(
        stream.next().await,
        Some(ProcessStreamEvent::Exited {
            exit_code: 7,
            exited: true
        })
    );
    assert_eq!(
        stream.next().await,
        Some(ProcessStreamEvent::Outcome(ProcessStreamOutcome::Completed))
    );
    assert_eq!(stream.next().await, None);
    {
        let mut state = fixture.state.lock().unwrap();
        state.runtime.finish = None;
        state.runtime.code = 15;
        state.runtime.exited = false;
    }
    let signaled = fixture
        .backend
        .stream_process(request())
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await;
    assert_eq!(
        signaled,
        vec![
            ProcessStreamEvent::Started { pid: 2001 },
            ProcessStreamEvent::Stdout(b"\0\xffstdout".to_vec()),
            ProcessStreamEvent::Stderr(b"\xfestderr".to_vec()),
            ProcessStreamEvent::Exited {
                exit_code: 15,
                exited: false
            },
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::Completed),
        ]
    );
    let state = fixture.state.lock().unwrap();
    assert_eq!(
        state.runtime.manifests[0]["args"],
        serde_json::json!(["a b'$(secret)\n"])
    );
    assert_eq!(state.runtime.manifests[0]["envs"]["EXACT"], "secret-value");
    for request in &state.requests {
        if let Some(command) = request.body["command"].as_str() {
            assert!(!command.contains("secret-value"));
            assert!(!command.contains("a b'$(secret)"));
        }
    }
    assert!(!state.runtime.actions.contains(&"terminate".to_owned()));
}

#[tokio::test]
async fn collected_accepts_output_above_sync_limits_and_stateless_keeps_combined_order() {
    let fixture = Fixture::start().await;
    {
        let mut state = fixture.state.lock().unwrap();
        state.runtime.stdout = vec![b'x'; 8 * 1024 * 1024 + 1];
        state.runtime.stderr.clear();
    }
    let req = request();
    let output = fixture
        .backend
        .run_process(BackendRunProcessRequest {
            sandbox_provider_ref: req.sandbox_provider_ref,
            command: req.command,
            args: req.args,
            cwd: req.cwd,
            envs: req.envs,
            stdout_limit: 9 * 1024 * 1024,
            stderr_limit: 0,
            deadline: Duration::from_secs(60),
        })
        .await
        .unwrap();
    assert_eq!(output.stdout.len(), 8 * 1024 * 1024 + 1);
    assert_eq!(output.exit_code, Some(0));
    assert!(output.exited);
    {
        let mut state = fixture.state.lock().unwrap();
        state.runtime.stdout = b"one".to_vec();
        state.runtime.stderr = b"two".to_vec();
    }
    let output = fixture
        .backend
        .read_only_exec(BackendReadOnlyExecRequest {
            sandbox_provider_ref: ProviderRef::new("bx_abcdefgh"),
            cwd: "/workspace".to_owned(),
            executable: "/bin/echo".to_owned(),
            args: Vec::new(),
            output_limit: 6,
            timeout: Duration::from_secs(30),
        })
        .await
        .unwrap();
    assert_eq!(output.bytes, b"onetwo");
    assert_eq!(output.exit_code, 0);
    let too_small = fixture
        .backend
        .read_only_exec(BackendReadOnlyExecRequest {
            sandbox_provider_ref: ProviderRef::new("bx_abcdefgh"),
            cwd: "/workspace".to_owned(),
            executable: "/bin/echo".to_owned(),
            args: Vec::new(),
            output_limit: 5,
            timeout: Duration::from_secs(30),
        })
        .await;
    assert!(matches!(
        too_small,
        Err(sandbox_interface::Error::ReadOnlyOutputTooLarge)
    ));
    assert_eq!(fixture.state.lock().unwrap().runtime.staging.len(), 0);
}
