//! Replacement-file payload identity regressions.

use std::{fs, process::Command};

use tempfile::tempdir;

use super::command;

#[test]
fn writer_rejects_same_size_staging_tampering() {
    let root = tempdir().expect("temporary write root");
    let stage = tempdir().expect("temporary staging root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    let target = root.path().join("src/lib.rs");
    let staged = stage.path().join("upload");
    let state = stage.path().join("state");
    fs::write(&target, b"old").expect("old target");
    fs::write(&staged, b"corruptions").expect("same-size altered upload");

    let command = command(
        root.path().to_string_lossy().into_owned(),
        "src/lib.rs".to_owned(),
        staged.to_string_lossy().into_owned(),
        ".sandbox-write-test".to_owned(),
        state.to_string_lossy().into_owned(),
        b"replacement".len(),
        "95713e9cbdd1dfcb2d4080c2537f418d43ca0da25f0d7d6631f4f7c97b89dc47".to_owned(),
    );
    let output = Command::new(command.command)
        .args(command.args)
        .output()
        .expect("run atomic writer");

    assert!(!output.status.success(), "altered payload was accepted");
    assert_eq!(fs::read(target).expect("unchanged target"), b"old");
}
