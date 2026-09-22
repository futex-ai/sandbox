//! Regression coverage for recovered write finalization and fence lifetime.

use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::Path,
    process::Command,
};

use tempfile::tempdir;

use super::{CleanupRequest, command};

const REPLACEMENT_DIGEST: &str = "95713e9cbdd1dfcb2d4080c2537f418d43ca0da25f0d7d6631f4f7c97b89dc47";

#[test]
fn exact_target_recovery_finishes_the_workload_handoff() {
    let root = tempdir().expect("temporary write root");
    let state_root = tempdir().expect("temporary state root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    let target = root.path().join("src/lib.rs");
    fs::write(&target, b"replacement").expect("replacement target");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).expect("target mode");
    let account_marker = state_root.path().join("account-observed");
    let file_sync_marker = state_root.path().join("file-sync-observed");
    let state = state_root.path().join("state");
    let mut cleanup = cleanup_command(root.path(), state_root.path(), true);
    let helper = cleanup.args.get_mut(3).expect("embedded cleanup helper");
    *helper = instrumented_helper(helper, &account_marker, &file_sync_marker);

    let output = Command::new(cleanup.command)
        .args(cleanup.args)
        .output()
        .expect("run instrumented write reconciler");

    assert_eq!(output.status.code(), Some(51));
    assert!(account_marker.exists(), "workload account was not resolved");
    assert!(file_sync_marker.exists(), "recovered file was not synced");
    assert_eq!(
        fs::metadata(target)
            .expect("target metadata")
            .permissions()
            .mode()
            & 0o7777,
        0o640
    );
    assert!(
        fs::symlink_metadata(state).is_ok(),
        "an uncertain writer must remain fenced"
    );
}

#[test]
fn exact_target_recovery_requires_a_non_root_workload_identity() {
    let root = tempdir().expect("temporary write root");
    let state_root = tempdir().expect("temporary state root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    fs::write(root.path().join("src/lib.rs"), b"replacement").expect("replacement target");
    let mut cleanup = cleanup_command(root.path(), state_root.path(), true);
    let workload_user = cleanup.args.len() - 2;
    cleanup.args[workload_user] = String::new();

    let output = Command::new(cleanup.command)
        .args(cleanup.args)
        .output()
        .expect("run write reconciler");

    assert_eq!(output.status.code(), Some(52));
    assert!(fs::symlink_metadata(state_root.path().join("state")).is_ok());
}

#[test]
fn disposable_cleanup_removes_its_revocation_marker() {
    let root = tempdir().expect("temporary write root");
    let state_root = tempdir().expect("temporary state root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    let state = state_root.path().join("state");

    let output = run_cleanup(root.path(), state_root.path(), false);

    assert!(output.status.success());
    assert!(
        fs::symlink_metadata(state).is_err(),
        "resolved disposable fence was retained"
    );
}

#[test]
fn uncertain_cleanup_retains_its_revocation_marker() {
    let root = tempdir().expect("temporary write root");
    let state_root = tempdir().expect("temporary state root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    let state = state_root.path().join("state");

    let output = run_cleanup(root.path(), state_root.path(), true);

    assert!(output.status.success());
    assert_eq!(
        fs::read_link(state)
            .expect("retained revocation marker")
            .to_string_lossy(),
        "revoked"
    );
}

#[test]
fn failed_disposable_cleanup_keeps_its_revocation_marker() {
    let root = tempdir().expect("temporary write root");
    let outside = tempdir().expect("outside directory");
    let state_root = tempdir().expect("temporary state root");
    symlink(outside.path(), root.path().join("src")).expect("linked parent");
    let state = state_root.path().join("state");

    let output = run_cleanup(root.path(), state_root.path(), false);

    assert!(!output.status.success());
    assert_eq!(
        fs::read_link(state)
            .expect("failure fence")
            .to_string_lossy(),
        "revoked"
    );
}

fn run_cleanup(root: &Path, state_root: &Path, retain_fence: bool) -> std::process::Output {
    let cleanup = cleanup_command(root, state_root, retain_fence);
    Command::new(cleanup.command)
        .args(cleanup.args)
        .output()
        .expect("run write reconciler")
}

fn cleanup_command(root: &Path, state_root: &Path, retain_fence: bool) -> super::ProcessCommand {
    command(CleanupRequest {
        root: root.to_string_lossy().into_owned(),
        path: "src/lib.rs".to_owned(),
        staging_path: state_root
            .join("missing-upload")
            .to_string_lossy()
            .into_owned(),
        temporary_directory: ".missing-temporary".to_owned(),
        state_root: state_root.to_string_lossy().into_owned(),
        state_path: "state".to_owned(),
        expected_size: b"replacement".len(),
        expected_digest: REPLACEMENT_DIGEST.to_owned(),
        workload_user: current_username(),
        retain_fence,
    })
}

fn instrumented_helper(helper: &str, account_marker: &Path, sync_marker: &Path) -> String {
    format!(
        "import os, pwd, stat\noriginal_getpwnam = pwd.getpwnam\ndef observed_getpwnam(name):\n open({:?}, 'wb').close()\n return original_getpwnam(name)\npwd.getpwnam = observed_getpwnam\noriginal_fsync = os.fsync\ndef observed_fsync(fd):\n if stat.S_ISREG(os.fstat(fd).st_mode):\n  open({:?}, 'wb').close()\n return original_fsync(fd)\nos.fsync = observed_fsync\n{}",
        account_marker.to_string_lossy(),
        sync_marker.to_string_lossy(),
        helper
    )
}

fn current_username() -> String {
    let output = Command::new("/usr/bin/id")
        .arg("-un")
        .output()
        .expect("resolve current username");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .expect("UTF-8 username")
        .trim()
        .to_owned()
}
