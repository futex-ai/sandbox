//! Local process smoke for the exact initialization supervisor command.

#![cfg(target_os = "linux")]

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, SystemTime};

use sandbox_e2b::{
    ControlSandboxReadAccess, E2bAdapterConfig, E2bControlApiMock, E2bProfile, E2bSandboxBackend,
    ProcessSplitOutput, ProcessTransportMock, SplitProcessCommand,
};
use sandbox_interface::{
    BackendResizeScreenStackRequest, Error, ProviderRef, Result, SandboxScreens, ScreenViewportSize,
};
use unimock::{MockFn, Unimock, matching};

/// After readiness, advance on each read in case the wait deadline is set
/// after the marker already exists.
const CLOCK_PROLOGUE: &str = r#"
import os
import sys
import time

clock_step = int(sys.argv[1]) / 1000 + 1.0
clock_value = 0.0

def readiness_monotonic():
    global clock_value
    if clock_value == 0.0:
        if not os.path.exists('started'):
            return 0.0
        open('clock-advanced', 'x').close()
    clock_value += clock_step
    return clock_value

time.monotonic = readiness_monotonic
"#;

const CLEANUP_HELPER: &str = r#"
import os, subprocess, sys

signal_name = os.environ['SCREEN_TEST_SIGNAL']
child = subprocess.Popen([sys.executable, '-c', '''
import os, pathlib, signal, sys

supervisor, signal_name = sys.argv[1:]
marker = 'signal-ready' if signal_name else 'started'
pathlib.Path(marker).write_text('started')
if signal_name:
    os.kill(int(supervisor), getattr(signal, signal_name))
sys.stdin.buffer.read(1)
pathlib.Path('late-mutation').write_text('late resize')
''', str(os.getppid()), signal_name])
child.wait()
"#;

const ACKNOWLEDGMENT_HELPER: &str = r#"
import json
import os
import signal
import sys

operation, width, height = sys.argv[1:]
assert operation == 'resize'
print(json.dumps({'version': 1, 'width': int(width), 'height': int(height)}), flush=True)
helper_exit = os.environ['SCREEN_TEST_HELPER_EXIT']
if helper_exit == 'SIGTERM':
    signal.signal(signal.SIGTERM, signal.SIG_DFL)
    signal.raise_signal(signal.SIGTERM)
else:
    sys.exit(int(helper_exit))
"#;

#[tokio::test]
async fn initialization_timeout_kills_the_helper_and_its_children_before_returning() {
    let directory = tempfile::tempdir().expect("fixture directory");
    let viewport = ScreenViewportSize::new(704, 768).expect("viewport");
    let result = resize_with_helper(
        viewport,
        directory.path(),
        CLEANUP_HELPER,
        ("SCREEN_TEST_SIGNAL", ""),
        |exit_code| {
            assert_ne!(
                exit_code,
                Some(0),
                "supervisor must report failure after the readiness clock advances"
            );
        },
    )
    .await;
    assert!(matches!(result, Err(Error::BackendUnavailable { .. })));
    assert!(
        directory.path().join("started").exists(),
        "fixture child must have started before expiry"
    );
    assert!(
        directory.path().join("clock-advanced").exists(),
        "supervisor must read the readiness clock after the fixture child starts"
    );
    assert!(
        !directory.path().join("late-mutation").exists(),
        "a child escaped the supervisor's process group"
    );
}

#[tokio::test]
async fn initialization_signal_kills_the_helper_and_its_children_before_returning() {
    let viewport = ScreenViewportSize::new(704, 768).expect("viewport");
    for signal in ["SIGTERM", "SIGALRM"] {
        let directory = tempfile::tempdir().expect("fixture directory");
        let result = resize_with_helper(
            viewport,
            directory.path(),
            CLEANUP_HELPER,
            ("SCREEN_TEST_SIGNAL", signal),
            move |exit_code| assert_eq!(exit_code, Some(124), "{signal}: supervisor exit code"),
        )
        .await;
        assert!(
            matches!(result, Err(Error::BackendUnavailable { .. })),
            "{signal}: expected BackendUnavailable, got {result:?}"
        );
        assert!(
            directory.path().join("signal-ready").exists(),
            "{signal}: fixture child must have started"
        );
        assert!(
            !directory.path().join("late-mutation").exists(),
            "{signal}: a child escaped the supervisor's process group"
        );
    }
}

#[tokio::test]
async fn initialization_resize_reports_the_supervised_helper_outcome() {
    let viewport = ScreenViewportSize::new(704, 768).expect("viewport");
    for (helper_exit, succeeds) in [("0", true), ("1", false), ("SIGTERM", false)] {
        let directory = tempfile::tempdir().expect("fixture directory");
        let result = resize_with_helper(
            viewport,
            directory.path(),
            ACKNOWLEDGMENT_HELPER,
            ("SCREEN_TEST_HELPER_EXIT", helper_exit),
            |_| {},
        )
        .await;
        let matches_expected = match &result {
            Ok(applied) => succeeds && *applied == viewport,
            Err(Error::BackendUnavailable { .. }) => !succeeds,
            Err(_) => false,
        };
        assert!(
            matches_expected,
            "helper exit {helper_exit}: expected success {succeeds}, got {result:?}"
        );
    }
}

fn run_supervisor(
    command: &SplitProcessCommand,
    working_directory: &Path,
    helper: &str,
    env: (&str, &str),
) -> ProcessSplitOutput {
    assert_eq!(command.command, "/usr/bin/python3");
    assert_eq!(&command.args[..3], ["-I", "-S", "-c"]);
    assert_eq!(command.args[5], "/usr/local/bin/sandbox-screen");
    let child = Command::new(&command.command)
        .args(&command.args[..3])
        .arg(format!("{CLOCK_PROLOGUE}{}", command.args[3]))
        .arg(&command.args[4])
        .args(["/usr/bin/python3", "-I", "-S", "-c", helper])
        .args(&command.args[6..])
        .current_dir(working_directory)
        .env(env.0, env.1)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run actual supervisor with a fixture helper");
    let output = wait_with_fixture_release(child);
    ProcessSplitOutput {
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code: Some(output.status.code().unwrap_or(-1)),
        exited: output.status.code().is_some(),
        ..ProcessSplitOutput::default()
    }
}

fn wait_with_fixture_release(mut child: Child) -> Output {
    let release = child.stdin.take().expect("fixture release pipe");
    let stdout = child.stdout.take().expect("supervisor stdout pipe");
    let stderr = child.stderr.take().expect("supervisor stderr pipe");
    thread::scope(|scope| {
        let stdout = scope.spawn(move || read_pipe(stdout));
        let stderr = scope.spawn(move || read_pipe(stderr));
        let (finished, completion) = mpsc::channel();
        scope.spawn(move || {
            let _ = finished.send(child.wait());
        });
        let status = completion.recv_timeout(Duration::from_secs(10));
        drop(release);
        let stdout = stdout.join().expect("read supervisor stdout");
        let stderr = stderr.join().expect("read supervisor stderr");
        let status = status
            .expect("resize supervisor hang guard expired before exit")
            .expect("wait for supervisor");
        Output {
            status,
            stdout,
            stderr,
        }
    })
}

fn read_pipe(mut pipe: impl Read) -> Vec<u8> {
    let mut bytes = Vec::new();
    pipe.read_to_end(&mut bytes)
        .expect("read fixture pipe to EOF");
    bytes
}

async fn resize_with_helper(
    viewport: ScreenViewportSize,
    working_directory: &Path,
    helper: &'static str,
    env: (&'static str, &'static str),
    check_exit: impl Fn(Option<i32>) + Send + Sync + 'static,
) -> Result<ScreenViewportSize> {
    let working_directory = working_directory.to_owned();
    let processes = Unimock::new(
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .answers_arc(Arc::new(move |_, _, command| {
                let output = run_supervisor(&command, &working_directory, helper, env);
                check_exit(output.exit_code);
                Ok(output)
            })),
    );
    let control = Unimock::new(
        E2bControlApiMock::get_sandbox_read_access
            .next_call(matching!(_))
            .returns(Ok(ControlSandboxReadAccess {
                sandbox_id: "provider".to_owned(),
                domain: "e2b.app".to_owned(),
                envd_access_token: "test".to_owned(),
            })),
    );
    let config = E2bAdapterConfig::new(
        "e2b",
        "https://api.e2b.app",
        "test",
        HashMap::from([(
            "general".to_owned(),
            E2bProfile {
                template: "test".to_owned(),
                allow_public_egress: false,
                denied_destinations: vec!["203.0.113.10/32".to_owned()],
            },
        )]),
        600,
    )
    .expect("valid config");
    let backend =
        E2bSandboxBackend::with_transports(config, Arc::new(control), Arc::new(processes));
    backend
        .resize_screen_stack(BackendResizeScreenStackRequest {
            sandbox_provider_ref: ProviderRef::new("provider"),
            viewport,
            deadline_at: Some((SystemTime::now() + Duration::from_secs(4)).into()),
        })
        .await
}
