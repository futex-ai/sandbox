//! Descriptor-relative image-cache cleanup regressions.

use std::{fs, os::unix::fs::symlink, process::Command};

use tempfile::tempdir;

use super::image_cache_cleanup::test_command;

#[test]
fn cache_cleanup_refuses_a_symlinked_parent_without_touching_its_target() {
    let home = tempdir().expect("temporary home");
    let outside = tempdir().expect("outside cache root");
    let protected = outside.path().join("_cacache/artifact");
    fs::create_dir(outside.path().join("_cacache")).expect("outside cache directory");
    fs::write(&protected, b"protected").expect("protected cache bytes");
    symlink(outside.path(), home.path().join(".npm")).expect("symlinked npm parent");

    let command = test_command([
        home.path().join(".cache").to_string_lossy().into_owned(),
        home.path()
            .join(".npm/_cacache")
            .to_string_lossy()
            .into_owned(),
    ]);
    let output = Command::new(command.command)
        .args(command.args)
        .output()
        .expect("run cache cleanup helper");

    assert!(!output.status.success());
    assert_eq!(fs::read(protected).expect("protected bytes"), b"protected");
}

#[test]
fn cache_cleanup_unlinks_a_child_symlink_without_traversing_its_target() {
    let home = tempdir().expect("temporary home");
    let outside = tempdir().expect("outside cache root");
    let cache = home.path().join(".cache");
    let protected = outside.path().join("artifact");
    fs::create_dir(&cache).expect("cache directory");
    fs::write(&protected, b"protected").expect("protected cache bytes");
    symlink(outside.path(), cache.join("linked")).expect("child symlink");

    let command = test_command([cache.to_string_lossy().into_owned()]);
    let output = Command::new(command.command)
        .args(command.args)
        .output()
        .expect("run cache cleanup helper");

    assert!(output.status.success());
    assert!(!cache.exists());
    assert_eq!(fs::read(protected).expect("protected bytes"), b"protected");
}
