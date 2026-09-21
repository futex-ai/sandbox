//! Descriptor-relative failed-write cleanup coverage.

use std::{fs, os::unix::fs::symlink, path::Path, process::Command, time::Duration};

use tempfile::tempdir;

use super::command;

#[test]
fn cleanup_removes_staging_and_destination_temporary_files() {
    let root = tempdir().expect("temporary write root");
    let stage = tempdir().expect("temporary staging root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    let staged = stage.path().join("upload");
    let temporary = root.path().join("src/.sandbox-write-test");
    fs::write(&staged, b"staged").expect("staged file");
    fs::write(&temporary, b"temporary").expect("destination temporary file");

    let output = run(root.path(), "src/lib.rs", &staged, ".sandbox-write-test");

    assert!(
        output.status.success(),
        "cleanup stderr: {:?}",
        output.stderr
    );
    assert!(!staged.exists());
    assert!(!temporary.exists());
}

#[test]
fn cleanup_does_not_follow_a_replaced_parent_directory() {
    let root = tempdir().expect("temporary write root");
    let outside = tempdir().expect("outside directory");
    let stage = tempdir().expect("temporary staging root");
    symlink(outside.path(), root.path().join("linked")).expect("linked parent");
    let staged = stage.path().join("upload");
    let outside_temporary = outside.path().join(".sandbox-write-test");
    fs::write(&staged, b"staged").expect("staged file");
    fs::write(&outside_temporary, b"protected").expect("outside temporary file");

    let output = run(root.path(), "linked/lib.rs", &staged, ".sandbox-write-test");

    assert!(!output.status.success());
    assert!(!staged.exists());
    assert_eq!(
        fs::read(outside_temporary).expect("outside file"),
        b"protected"
    );
}

#[test]
fn cleanup_second_pass_removes_a_late_temporary_file() {
    let root = tempdir().expect("temporary write root");
    let stage = tempdir().expect("temporary staging root");
    fs::create_dir(root.path().join("src")).expect("nested directory");
    let staged = stage.path().join("upload");
    fs::write(&staged, b"staged").expect("staged file");
    let temporary = root.path().join("src/.sandbox-write-test");
    let late_temporary = temporary.clone();
    let writer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(1));
        fs::write(late_temporary, b"late").expect("late temporary file");
    });

    let output = run(root.path(), "src/lib.rs", &staged, ".sandbox-write-test");
    writer.join().expect("late writer should finish");

    assert!(
        output.status.success(),
        "cleanup stderr: {:?}",
        output.stderr
    );
    assert!(!staged.exists());
    assert!(!temporary.exists());
}

fn run(root: &Path, path: &str, staged: &Path, temporary_name: &str) -> std::process::Output {
    let command = command(
        root.to_string_lossy().into_owned(),
        path.to_owned(),
        staged.to_string_lossy().into_owned(),
        temporary_name.to_owned(),
    );
    Command::new(command.command)
        .args(command.args)
        .output()
        .expect("run failed-write cleanup")
}
