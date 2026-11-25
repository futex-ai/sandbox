//! Local process smoke for the exact initialization supervisor command.

#![cfg(target_os = "linux")]

#[path = "support/resize_supervisor.rs"]
mod resize_supervisor;

use sandbox_interface::{Error, ScreenViewportSize};

use self::resize_supervisor::{python_helper_argv, resize_with_helper};

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
if os.path.exists('wait-for-release'):
    open('acknowledged', 'x').close()
    sys.stdin.buffer.read(1)
helper_exit = os.environ['SCREEN_TEST_HELPER_EXIT']
if helper_exit == 'SIGTERM':
    signal.signal(signal.SIGTERM, signal.SIG_DFL)
    signal.raise_signal(signal.SIGTERM)
else:
    sys.exit(int(helper_exit))
"#;

/// Inject a polling fault after the helper flushes its acknowledgment.
const WAITID_FAULT_PROLOGUE: &str = r#"
import os

open('wait-for-release', 'x').close()
original_waitid = os.waitid

def waitid_with_fault(*args):
    if os.path.exists('acknowledged'):
        os.rename('acknowledged', 'waitid-fault')
        raise RuntimeError('fixture fault while polling resize helper')
    return original_waitid(*args)

os.waitid = waitid_with_fault
"#;

#[tokio::test]
async fn initialization_timeout_kills_the_helper_and_its_children_before_returning() {
    let directory = tempfile::tempdir().expect("fixture directory");
    let viewport = ScreenViewportSize::new(704, 768).expect("viewport");
    let result = resize_with_helper(
        viewport,
        directory.path(),
        python_helper_argv(CLEANUP_HELPER),
        Some(("SCREEN_TEST_SIGNAL", "")),
        "",
        |output| {
            let exit_code = output.exit_code;
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
            python_helper_argv(CLEANUP_HELPER),
            Some(("SCREEN_TEST_SIGNAL", signal)),
            "",
            move |output| {
                let exit_code = output.exit_code;
                assert_eq!(exit_code, Some(124), "{signal}: supervisor exit code");
            },
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
    for (helper_exit, succeeds, supervisor_exit) in
        [("0", true, 0), ("1", false, 3), ("SIGTERM", false, 3)]
    {
        let directory = tempfile::tempdir().expect("fixture directory");
        let result = resize_with_helper(
            viewport,
            directory.path(),
            python_helper_argv(ACKNOWLEDGMENT_HELPER),
            Some(("SCREEN_TEST_HELPER_EXIT", helper_exit)),
            "",
            move |output| {
                let exit_code = output.exit_code;
                assert_eq!(
                    exit_code,
                    Some(supervisor_exit),
                    "unexpected supervisor exit code for helper exit {helper_exit}"
                );
            },
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

#[tokio::test]
async fn initialization_with_a_missing_helper_leaves_cleanup_unconfirmed() {
    let directory = tempfile::tempdir().expect("fixture directory");
    let viewport = ScreenViewportSize::new(704, 768).expect("viewport");
    let missing_helper = directory.path().join("missing-screen-helper");
    let result = resize_with_helper(
        viewport,
        directory.path(),
        vec![missing_helper.into_os_string()],
        None,
        "",
        |output| {
            assert_eq!(
                output.exit_code,
                Some(1),
                "expected Python's uncaught-error exit status for a missing helper"
            );
        },
    )
    .await;
    assert!(
        matches!(result, Err(Error::ScreenViewportResizeUnconfirmed)),
        "expected ScreenViewportResizeUnconfirmed for a missing helper, got {result:?}"
    );
}

#[tokio::test]
async fn initialization_with_a_supervisor_polling_fault_leaves_cleanup_unconfirmed() {
    let directory = tempfile::tempdir().expect("fixture directory");
    let viewport = ScreenViewportSize::new(704, 768).expect("viewport");
    let result = resize_with_helper(
        viewport,
        directory.path(),
        python_helper_argv(ACKNOWLEDGMENT_HELPER),
        Some(("SCREEN_TEST_HELPER_EXIT", "0")),
        WAITID_FAULT_PROLOGUE,
        |output| {
            assert_eq!(
                output.stdout.as_slice(),
                b"{\"version\": 1, \"width\": 704, \"height\": 768}\n",
                "helper must flush a valid acknowledgment before the polling fault"
            );
            assert!(
                !matches!(output.exit_code, Some(0 | 3 | 124)),
                "expected a supervisor exit outside 0, 3, and 124 after the polling fault, got {:?}",
                output.exit_code
            );
        },
    )
    .await;
    assert!(
        directory.path().join("waitid-fault").exists(),
        "supervisor must reach the injected waitid fault"
    );
    assert!(
        matches!(result, Err(Error::ScreenViewportResizeUnconfirmed)),
        "expected ScreenViewportResizeUnconfirmed after the polling fault, got {result:?}"
    );
}
