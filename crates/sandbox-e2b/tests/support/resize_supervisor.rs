//! Local fixtures for the deadline resize supervisor.

use std::collections::HashMap;
use std::ffi::OsString;
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
    BackendResizeScreenStackRequest, ProviderRef, Result, SandboxScreens, ScreenViewportSize,
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

/// The child inherits the helper's process group and stdio. It closes the
/// readiness pipe after writing its marker, then waits for stdin release.
const CHILD_FIXTURE: &str = r#"
import os
import subprocess
import sys

child_script = '''
import os, pathlib, signal, sys

marker, supervisor, signal_name, ready_fd = sys.argv[1:]
pathlib.Path(marker).write_text('started')
os.close(int(ready_fd))
if signal_name:
    os.kill(int(supervisor), getattr(signal, signal_name))
sys.stdin.buffer.read(1)
pathlib.Path('late-mutation').write_text('late resize')
'''

def start_child(marker, signal_name=''):
    read_fd, write_fd = os.pipe()
    with os.fdopen(read_fd, 'rb') as ready, os.fdopen(write_fd, 'wb') as writer:
        child = subprocess.Popen(
            [
                sys.executable, '-c', child_script, marker,
                str(os.getppid()), signal_name, str(write_fd),
            ],
            pass_fds=(write_fd,),
        )
        writer.close()
        ready.read(1)
    return child
"#;

/// Build the helper argv with Python's isolated startup flags.
pub(super) fn python_helper_argv(helper: &str) -> Vec<OsString> {
    ["/usr/bin/python3", "-I", "-S", "-c", helper]
        .into_iter()
        .map(OsString::from)
        .collect()
}

/// Build the argv for a helper that starts the shared child fixture.
pub(super) fn python_helper_argv_with_child(helper: &str) -> Vec<OsString> {
    python_helper_argv(&format!("{CHILD_FIXTURE}{helper}"))
}

fn run_supervisor(
    command: &SplitProcessCommand,
    working_directory: &Path,
    helper_argv: &[OsString],
    env: Option<(&str, &str)>,
    prologue: &str,
) -> ProcessSplitOutput {
    assert_eq!(command.command, "/usr/bin/python3");
    assert_eq!(&command.args[..3], ["-I", "-S", "-c"]);
    assert_eq!(command.args[5], "/usr/local/bin/sandbox-screen");
    let child = Command::new(&command.command)
        .args(&command.args[..3])
        .arg(format!("{CLOCK_PROLOGUE}{prologue}{}", command.args[3]))
        .arg(&command.args[4])
        .args(helper_argv)
        .args(&command.args[6..])
        .current_dir(working_directory)
        .envs(env)
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

/// Run the supervisor locally as the resize request's process transport.
pub(super) async fn resize_with_helper(
    viewport: ScreenViewportSize,
    working_directory: &Path,
    helper_argv: Vec<OsString>,
    env: Option<(&'static str, &'static str)>,
    prologue: &'static str,
    check_output: impl Fn(&ProcessSplitOutput) + Send + Sync + 'static,
) -> Result<ScreenViewportSize> {
    let working_directory = working_directory.to_owned();
    let processes = Unimock::new(
        ProcessTransportMock::run_split
            .next_call(matching!(_, _))
            .answers_arc(Arc::new(move |_, _, command| {
                let output =
                    run_supervisor(&command, &working_directory, &helper_argv, env, prologue);
                check_output(&output);
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
