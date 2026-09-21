//! Exact terminal transcript limit coverage.

use std::{
    fs,
    io::Write,
    os::unix::fs::symlink,
    process::{Command, Stdio},
};

use tempfile::tempdir;

use super::TERMINAL_WRAPPER;

#[test]
fn terminal_wrapper_enforces_non_aligned_byte_limits_exactly() {
    for limit in [64, 4097] {
        let directory = tempdir().expect("temporary transcript directory");
        let transcript = directory.path().join("terminal.log");
        let mut child = start_wrapper(&transcript, limit);
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
            u64::try_from(limit).expect("test limit")
        );
    }
}

#[test]
fn terminal_wrapper_does_not_follow_an_existing_log_symlink() {
    let directory = tempdir().expect("temporary transcript directory");
    let outside = tempdir().expect("outside transcript directory");
    let transcript = directory.path().join("terminal.log");
    let protected = outside.path().join("protected.log");
    fs::write(&protected, b"protected").expect("protected file");
    symlink(&protected, &transcript).expect("transcript symlink");
    let mut child = start_wrapper(&transcript, 1024 * 1024);
    child
        .stdin
        .take()
        .expect("terminal wrapper stdin")
        .write_all(b"echo captured\nexit\n")
        .expect("write terminal command");

    let status = child.wait().expect("wait for terminal wrapper");

    assert!(status.success(), "terminal wrapper did not exit normally");
    assert_eq!(fs::read(&protected).expect("protected file"), b"protected");
    assert!(
        !fs::symlink_metadata(&transcript)
            .expect("replacement transcript")
            .file_type()
            .is_symlink()
    );
}

#[test]
fn terminal_wrapper_rejects_a_symlinked_parent_directory() {
    let directory = tempdir().expect("temporary transcript directory");
    let outside = tempdir().expect("outside transcript directory");
    let linked_parent = directory.path().join("linked");
    symlink(outside.path(), &linked_parent).expect("parent symlink");
    let transcript = linked_parent.join("terminal.log");
    let mut child = start_wrapper(&transcript, 1024 * 1024);

    let status = child.wait().expect("wait for terminal wrapper");

    assert!(!status.success(), "symlinked parent should fail closed");
    assert!(!outside.path().join("terminal.log").exists());
}

#[test]
fn terminal_wrapper_exits_normally_below_the_limit() {
    let directory = tempdir().expect("temporary transcript directory");
    let transcript = directory.path().join("terminal.log");
    let mut child = start_wrapper(&transcript, 1024 * 1024);
    child
        .stdin
        .take()
        .expect("terminal wrapper stdin")
        .write_all(b"echo complete\nexit\n")
        .expect("write terminal command");

    let status = child.wait().expect("wait for terminal wrapper");

    assert!(status.success(), "terminal wrapper did not exit normally");
    let transcript_size = std::fs::metadata(transcript)
        .expect("completed transcript")
        .len();
    assert!(transcript_size > 0 && transcript_size < 1024 * 1024);
}

fn start_wrapper(path: &std::path::Path, limit: usize) -> std::process::Child {
    Command::new("/usr/bin/timeout")
        .args(["5s", "/usr/bin/python3", "-c", TERMINAL_WRAPPER])
        .arg(path)
        .arg(limit.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start terminal wrapper")
}
