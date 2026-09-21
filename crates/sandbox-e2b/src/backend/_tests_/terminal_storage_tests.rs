//! Behavioral coverage for non-following terminal storage helpers.

use std::{fs, os::unix::fs::symlink, process::Command};

use tempfile::tempdir;

use super::{cleanup_command, directory_command};
use crate::ProcessCommand;

#[test]
fn directory_creation_rejects_an_intermediate_symlink() {
    let root = tempdir().expect("temporary path root");
    let outside = tempdir().expect("outside directory");
    symlink(outside.path(), root.path().join("sandbox")).expect("intermediate symlink");
    let target = root.path().join("sandbox/terminals");

    let output = run(directory_command(&target.to_string_lossy()));

    assert!(!output.status.success());
    assert!(!outside.path().join("terminals").exists());
}

#[test]
fn directory_creation_builds_a_normal_nested_path() {
    let root = tempdir().expect("temporary path root");
    let target = root.path().join("sandbox/terminals");

    let output = run(directory_command(&target.to_string_lossy()));

    assert!(output.status.success());
    assert!(target.is_dir());
}

#[test]
fn cleanup_rejects_an_intermediate_symlink_without_touching_its_target() {
    let root = tempdir().expect("temporary path root");
    let outside = tempdir().expect("outside directory");
    fs::create_dir(outside.path().join("terminals")).expect("outside terminals");
    let protected = outside.path().join("terminals/protected");
    fs::write(&protected, b"protected").expect("protected file");
    symlink(outside.path(), root.path().join("sandbox")).expect("intermediate symlink");
    let target = root.path().join("sandbox/terminals");

    let output = run(cleanup_command(&[&target.to_string_lossy()]));

    assert!(!output.status.success());
    assert_eq!(fs::read(protected).expect("protected bytes"), b"protected");
}

#[test]
fn cleanup_removes_child_symlinks_without_following_them() {
    let root = tempdir().expect("temporary path root");
    let outside = tempdir().expect("outside directory");
    let target = root.path().join("sandbox/terminals");
    fs::create_dir_all(&target).expect("terminal directory");
    let protected = outside.path().join("protected");
    fs::write(&protected, b"protected").expect("protected file");
    symlink(outside.path(), target.join("linked")).expect("child symlink");
    fs::write(target.join("transcript.log"), b"transcript").expect("terminal transcript");

    let output = run(cleanup_command(&[&target.to_string_lossy()]));

    assert!(output.status.success());
    assert!(!target.exists());
    assert_eq!(fs::read(protected).expect("protected bytes"), b"protected");
}

fn run(command: ProcessCommand) -> std::process::Output {
    Command::new(command.command)
        .args(command.args)
        .output()
        .expect("run terminal storage helper")
}
