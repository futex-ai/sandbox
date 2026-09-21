//! Atomic regular-file helper coverage.

use std::{fs, process::Command};

use tempfile::tempdir;

use crate::process::ProcessRegularFileRequest;

use super::{command, decode};

#[test]
fn reader_uses_one_direct_descriptor_relative_operation() {
    let command = command(request("/workspace/repo", "src/lib.rs"));

    assert_eq!(command.command, "/usr/bin/python3");
    assert_eq!(command.args[0], "-c");
    assert_eq!(command.args[2], "/workspace/repo");
    assert_eq!(command.args[3], "src/lib.rs");
    assert!(command.args[1].contains("dir_fd=directory"));
    assert!(command.args[1].contains("os.O_NOFOLLOW"));
    assert!(command.args[1].contains("os.fstat(opened)"));
    assert!(command.args[1].contains("os.pread(opened"));
    assert!(!command.args[1].contains("realpath"));
    assert_ne!(command.command, "/bin/sh");
}

#[test]
fn reader_returns_bytes_from_the_opened_regular_file() {
    let root = tempdir().expect("temporary read root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    fs::write(root.path().join("src/lib.rs"), b"abcdef").expect("regular file");

    let output = run(request(path(root.path()), "src/lib.rs"));

    assert!(output.status.success());
    let result = decode(&output.stdout, 4).expect("atomic file response");
    assert_eq!(result.bytes, b"abcd");
    assert_eq!(result.total_size, 6);
}

#[test]
fn reader_rejects_a_leaf_replaced_by_a_symlink_before_open() {
    let root = tempdir().expect("temporary read root");
    let adjacent = tempdir().expect("adjacent directory");
    let target = root.path().join("visible.txt");
    fs::write(&target, b"visible").expect("initial regular file");
    fs::write(adjacent.path().join("secret.txt"), b"secret").expect("adjacent secret");
    let command = command(request(path(root.path()), "visible.txt"));
    fs::remove_file(&target).expect("remove authorized file");
    std::os::unix::fs::symlink(adjacent.path().join("secret.txt"), &target)
        .expect("replace with symlink");

    let output = Command::new(&command.command)
        .args(&command.args)
        .output()
        .expect("run atomic reader");

    assert_eq!(output.status.code(), Some(46));
    assert!(output.stdout.is_empty());
}

fn request(root: impl Into<String>, path: impl Into<String>) -> ProcessRegularFileRequest {
    ProcessRegularFileRequest {
        root: root.into(),
        path: path.into(),
        offset: 0,
        max_bytes: 4,
    }
}

fn run(request: ProcessRegularFileRequest) -> std::process::Output {
    let command = command(request);
    Command::new(&command.command)
        .args(&command.args)
        .output()
        .expect("run atomic reader")
}

fn path(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}
