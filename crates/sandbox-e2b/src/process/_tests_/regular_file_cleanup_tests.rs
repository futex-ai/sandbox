//! Descriptor-relative write revocation and reconciliation coverage.

use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::Path,
    process::Command,
};

use tempfile::tempdir;

use super::{CleanupRequest, command};

const REPLACEMENT_DIGEST: &str = "95713e9cbdd1dfcb2d4080c2537f418d43ca0da25f0d7d6631f4f7c97b89dc47";

#[test]
fn cleanup_revokes_the_writer_and_removes_temporary_files() {
    let root = tempdir().expect("temporary write root");
    let stage = tempdir().expect("temporary staging root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    let staged = stage.path().join("upload");
    let state = stage.path().join("state");
    let temporary = root.path().join("src/.sandbox-write-test");
    fs::write(&staged, b"staged").expect("staged file");
    private_temporary(&temporary, b"temporary");

    let output = run(
        root.path(),
        "src/lib.rs",
        &staged,
        ".sandbox-write-test",
        &state,
        6,
    );

    assert!(
        output.status.success(),
        "cleanup stderr: {:?}",
        output.stderr
    );
    assert!(!staged.exists());
    assert!(!temporary.exists());
    assert_eq!(
        fs::read_link(state)
            .expect("revocation marker")
            .to_string_lossy(),
        "revoked"
    );
}

#[test]
fn cleanup_does_not_follow_a_replaced_parent_directory() {
    let root = tempdir().expect("temporary write root");
    let outside = tempdir().expect("outside directory");
    let stage = tempdir().expect("temporary staging root");
    symlink(outside.path(), root.path().join("linked")).expect("linked parent");
    let staged = stage.path().join("upload");
    let state = stage.path().join("state");
    let outside_temporary = outside.path().join(".sandbox-write-test");
    fs::write(&staged, b"staged").expect("staged file");
    private_temporary(&outside_temporary, b"protected");

    let output = run(
        root.path(),
        "linked/lib.rs",
        &staged,
        ".sandbox-write-test",
        &state,
        6,
    );

    assert!(!output.status.success());
    assert!(!staged.exists());
    assert_eq!(
        fs::read(outside_temporary.join("payload")).expect("outside file"),
        b"protected"
    );
}

#[test]
fn cleanup_finishes_a_writer_that_won_the_atomic_commit_claim() {
    let root = tempdir().expect("temporary write root");
    let stage = tempdir().expect("temporary staging root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    let staged = stage.path().join("missing-upload");
    let state = stage.path().join("state");
    let target = root.path().join("src/lib.rs");
    let temporary = root.path().join("src/.sandbox-write-test");
    fs::write(&target, b"old").expect("old target");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).expect("target mode");
    private_temporary(&temporary, b"replacement");
    symlink(commit_claim(&target), &state).expect("commit claim");

    let output = run(
        root.path(),
        "src/lib.rs",
        &staged,
        ".sandbox-write-test",
        &state,
        b"replacement".len(),
    );

    assert_eq!(output.status.code(), Some(51));
    assert_eq!(fs::read(target).expect("reconciled target"), b"replacement");
    assert_eq!(
        fs::metadata(root.path().join("src/lib.rs"))
            .expect("reconciled metadata")
            .permissions()
            .mode()
            & 0o7777,
        0o640
    );
    assert!(!temporary.exists());
}

#[test]
fn cleanup_rejects_a_commit_claim_for_different_same_size_bytes() {
    let root = tempdir().expect("temporary write root");
    let stage = tempdir().expect("temporary staging root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    let state = stage.path().join("state");
    let target = root.path().join("src/lib.rs");
    let temporary = root.path().join("src/.sandbox-write-test");
    fs::write(&target, b"old").expect("old target");
    private_temporary(&temporary, b"corruptions");
    symlink(commit_claim(&target), &state).expect("valid commit claim");

    let output = run(
        root.path(),
        "src/lib.rs",
        &stage.path().join("missing-upload"),
        ".sandbox-write-test",
        &state,
        b"replacement".len(),
    );

    assert_ne!(output.status.code(), Some(51));
    assert_eq!(fs::read(target).expect("unchanged target"), b"old");
    assert!(!temporary.exists());
}

#[test]
fn cleanup_rejects_a_workload_accessible_temporary_directory() {
    let root = tempdir().expect("temporary write root");
    let stage = tempdir().expect("temporary staging root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    let state = stage.path().join("state");
    let target = root.path().join("src/lib.rs");
    let temporary = root.path().join("src/.sandbox-write-test");
    fs::write(&target, b"old").expect("old target");
    private_temporary(&temporary, b"replacement");
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o755))
        .expect("workload-accessible temporary permissions");
    symlink(commit_claim(&target), &state).expect("commit claim");

    let output = run(
        root.path(),
        "src/lib.rs",
        &stage.path().join("missing-upload"),
        ".sandbox-write-test",
        &state,
        b"replacement".len(),
    );

    assert_ne!(output.status.code(), Some(51));
    assert_eq!(fs::read(target).expect("unchanged target"), b"old");
    assert_eq!(
        fs::read(temporary.join("payload")).expect("untrusted temporary payload"),
        b"replacement"
    );
}

#[test]
fn cleanup_recovers_a_commit_after_the_writer_removed_its_state() {
    let root = tempdir().expect("temporary write root");
    let stage = tempdir().expect("temporary staging root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    let state = stage.path().join("missing-state");
    let target = root.path().join("src/lib.rs");
    fs::write(&target, b"replacement").expect("committed target");

    let output = run(
        root.path(),
        "src/lib.rs",
        &stage.path().join("missing-upload"),
        ".missing-temporary",
        &state,
        b"replacement".len(),
    );

    assert_eq!(output.status.code(), Some(51));
    assert_eq!(fs::read(target).expect("committed target"), b"replacement");
}

#[test]
fn cleanup_syncs_the_directory_before_confirming_an_existing_target() {
    let root = tempdir().expect("temporary write root");
    let stage = tempdir().expect("temporary staging root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    let state = stage.path().join("state");
    let marker = stage.path().join("fsync-observed");
    fs::write(root.path().join("src/lib.rs"), b"replacement").expect("replacement target");
    symlink(commit_claim(&root.path().join("src/lib.rs")), &state).expect("commit claim");
    let mut command = command(CleanupRequest {
        root: root.path().to_string_lossy().into_owned(),
        path: "src/lib.rs".to_owned(),
        staging_path: stage
            .path()
            .join("missing-upload")
            .to_string_lossy()
            .into_owned(),
        temporary_directory: ".missing-temporary".to_owned(),
        state_root: stage.path().to_string_lossy().into_owned(),
        state_path: "state".to_owned(),
        expected_size: b"replacement".len(),
        expected_digest: REPLACEMENT_DIGEST.to_owned(),
    });
    let helper = command.args.get_mut(3).expect("embedded cleanup helper");
    *helper = format!(
        "import os\noriginal_fsync = os.fsync\ndef observed_fsync(fd):\n open({:?}, 'wb').close()\n return original_fsync(fd)\nos.fsync = observed_fsync\n{}",
        marker.to_string_lossy(),
        helper
    );

    let output = Command::new(command.command)
        .args(command.args)
        .output()
        .expect("run instrumented write reconciler");

    assert_eq!(output.status.code(), Some(51));
    assert!(marker.exists(), "committed target was not made durable");
}

fn run(
    root: &Path,
    path: &str,
    staged: &Path,
    temporary_directory: &str,
    state: &Path,
    expected_size: usize,
) -> std::process::Output {
    let command = command(CleanupRequest {
        root: root.to_string_lossy().into_owned(),
        path: path.to_owned(),
        staging_path: staged.to_string_lossy().into_owned(),
        temporary_directory: temporary_directory.to_owned(),
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
    });
    Command::new(command.command)
        .args(command.args)
        .output()
        .expect("run write reconciler")
}

fn commit_claim(target: &Path) -> String {
    let metadata = fs::metadata(target).expect("target metadata");
    format!(
        "commit:{REPLACEMENT_DIGEST}:{}:{}:{}",
        metadata.uid(),
        metadata.gid(),
        metadata.permissions().mode() & 0o7777
    )
}

fn private_temporary(path: &Path, bytes: &[u8]) {
    fs::create_dir(path).expect("private temporary directory");
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .expect("private temporary permissions");
    fs::write(path.join("payload"), bytes).expect("private temporary payload");
}
