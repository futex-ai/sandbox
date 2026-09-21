//! Atomic regular-file replacement helper coverage.

use std::{fs, os::unix::fs::symlink, path::Path, process::Command};

use tempfile::tempdir;

use super::command;

#[test]
fn writer_replaces_a_regular_file_through_directory_descriptors() {
    let root = tempdir().expect("temporary write root");
    let stage = tempdir().expect("temporary staging root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    fs::write(root.path().join("src/lib.rs"), b"old").expect("existing file");
    let staged = stage.path().join("upload");
    fs::write(&staged, b"replacement").expect("staged bytes");

    let output = run(root.path(), "src/lib.rs", &staged, b"replacement".len());

    assert!(
        output.status.success(),
        "helper stderr: {:?}",
        output.stderr
    );
    assert_eq!(
        fs::read(root.path().join("src/lib.rs")).expect("replacement file"),
        b"replacement"
    );
    assert!(!staged.exists());
}

#[test]
fn writer_rejects_a_leaf_symlink_without_changing_its_target() {
    let root = tempdir().expect("temporary write root");
    let outside = tempdir().expect("outside directory");
    let stage = tempdir().expect("temporary staging root");
    let outside_file = outside.path().join("outside.txt");
    fs::write(&outside_file, b"protected").expect("outside file");
    symlink(&outside_file, root.path().join("target.txt")).expect("leaf symlink");
    let staged = stage.path().join("upload");
    fs::write(&staged, b"replacement").expect("staged bytes");

    let output = run(root.path(), "target.txt", &staged, b"replacement".len());

    assert_eq!(output.status.code(), Some(46));
    assert_eq!(fs::read(outside_file).expect("outside file"), b"protected");
}

#[test]
fn writer_rejects_a_symlinked_parent_without_writing_outside_root() {
    let root = tempdir().expect("temporary write root");
    let outside = tempdir().expect("outside directory");
    let stage = tempdir().expect("temporary staging root");
    symlink(outside.path(), root.path().join("linked")).expect("parent symlink");
    let staged = stage.path().join("upload");
    fs::write(&staged, b"replacement").expect("staged bytes");

    let output = run(
        root.path(),
        "linked/target.txt",
        &staged,
        b"replacement".len(),
    );

    assert_eq!(output.status.code(), Some(46));
    assert!(!outside.path().join("target.txt").exists());
}

fn run(root: &Path, path: &str, staged: &Path, expected_size: usize) -> std::process::Output {
    let command = command(
        root.to_string_lossy().into_owned(),
        path.to_owned(),
        staged.to_string_lossy().into_owned(),
        ".sandbox-write-test".to_owned(),
        expected_size,
    );
    Command::new(command.command)
        .args(command.args)
        .output()
        .expect("run atomic writer")
}
