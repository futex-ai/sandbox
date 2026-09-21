//! Exact terminal transcript limit coverage.

use std::{
    io::Write,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use tempfile::tempdir;

use super::TERMINAL_WRAPPER;

#[test]
fn terminal_wrapper_enforces_non_aligned_byte_limits_exactly() {
    for limit in [64, 4097] {
        let directory = tempdir().expect("temporary transcript directory");
        let transcript = directory.path().join("terminal.log");
        let mut child = Command::new("/usr/bin/timeout")
            .args(["5s", "/bin/bash", "-c", TERMINAL_WRAPPER])
            .env("SANDBOX_TERMINAL_LOG_LIMIT", limit.to_string())
            .env("SANDBOX_TERMINAL_LOG_PATH", &transcript)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start terminal wrapper");
        child
            .stdin
            .take()
            .expect("terminal wrapper stdin")
            .write_all(b"printf '%010000d' 0\nexit\n")
            .expect("write terminal command");

        let status = child.wait().expect("wait for bounded terminal wrapper");

        assert_ne!(status.code(), Some(124), "terminal wrapper timed out");
        assert_eq!(
            std::fs::metadata(&transcript)
                .expect("bounded transcript")
                .len(),
            limit
        );
    }
}

#[test]
fn terminal_wrapper_exits_normally_below_the_limit_and_removes_its_pipe() {
    let directory = tempdir().expect("temporary transcript directory");
    let transcript = directory.path().join("terminal.log");
    let transcript_pipe = directory.path().join("terminal.log.pipe");
    let mut child = Command::new("/usr/bin/timeout")
        .args(["5s", "/bin/bash", "-c", TERMINAL_WRAPPER])
        .env("SANDBOX_TERMINAL_LOG_LIMIT", "4096")
        .env("SANDBOX_TERMINAL_LOG_PATH", &transcript)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start terminal wrapper");
    child
        .stdin
        .take()
        .expect("terminal wrapper stdin")
        .write_all(b"echo complete\nexit\n")
        .expect("write terminal command");

    let status = child.wait().expect("wait for terminal wrapper");
    let cleanup_deadline = Instant::now() + Duration::from_secs(1);
    while transcript_pipe.exists() && Instant::now() < cleanup_deadline {
        thread::sleep(Duration::from_millis(10));
    }

    assert!(status.success(), "terminal wrapper did not exit normally");
    assert!(!transcript_pipe.exists(), "transcript pipe was not removed");
    let transcript_size = std::fs::metadata(transcript)
        .expect("completed transcript")
        .len();
    assert!(transcript_size > 0 && transcript_size < 4096);
}
