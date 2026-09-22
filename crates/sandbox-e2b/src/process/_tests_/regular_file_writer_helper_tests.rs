//! Descriptor-relative atomic writer helper coverage.

use std::{
    fs,
    os::unix::fs::symlink,
    path::Path,
    process::{Command, Output},
};

use tempfile::tempdir;

use super::{WriteAttempt, command};

const REPLACEMENT_DIGEST: &str = "95713e9cbdd1dfcb2d4080c2537f418d43ca0da25f0d7d6631f4f7c97b89dc47";

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
    assert!(fs::symlink_metadata(staged.with_extension("state")).is_err());
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

#[test]
fn writer_cannot_replace_after_cleanup_claims_revocation() {
    let root = tempdir().expect("temporary write root");
    let stage = tempdir().expect("temporary staging root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    let target = root.path().join("src/lib.rs");
    let staged = stage.path().join("upload");
    let state = stage.path().join("state");
    fs::write(&target, b"old").expect("old target");
    fs::write(&staged, b"replacement").expect("staged bytes");
    symlink("revoked", &state).expect("revocation claim");

    let output = run_with_state(
        root.path(),
        "src/lib.rs",
        &staged,
        &state,
        b"replacement".len(),
    );

    assert_eq!(output.status.code(), Some(50));
    assert_eq!(fs::read(target).expect("unchanged target"), b"old");
}

fn run(root: &Path, path: &str, staged: &Path, expected_size: usize) -> Output {
    run_with_state(
        root,
        path,
        staged,
        &staged.with_extension("state"),
        expected_size,
    )
}

fn run_with_state(
    root: &Path,
    path: &str,
    staged: &Path,
    state: &Path,
    expected_size: usize,
) -> Output {
    let attempt = WriteAttempt {
        root: root.to_string_lossy().into_owned(),
        path: path.to_owned(),
        staging_path: staged.to_string_lossy().into_owned(),
        temporary_name: ".sandbox-write-test".to_owned(),
        state_root: state
            .parent()
            .expect("state parent")
            .to_string_lossy()
            .into_owned(),
        state_path: state
            .file_name()
            .expect("state name")
            .to_string_lossy()
            .into_owned(),
        expected_size,
        expected_digest: REPLACEMENT_DIGEST.to_owned(),
        workload_user: String::new(),
    };
    let command = command(&attempt);
    Command::new(command.command)
        .args(command.args)
        .output()
        .expect("run atomic writer")
}
