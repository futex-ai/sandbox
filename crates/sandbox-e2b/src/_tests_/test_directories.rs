//! Owner-only temporary directories for helper tests, independent of umask.
//!
//! Trusted helpers reject group- or world-writable roots. `tempfile::tempdir`
//! applies the process umask to mode `0o777`, so a common `0002` umask makes
//! its directories group-writable and the helpers correctly refuse them.

use std::{fs::Permissions, io, os::unix::fs::PermissionsExt};

use tempfile::{Builder, TempDir};

/// Creates a temporary directory with explicit owner-only `0o700` permissions.
pub(crate) fn private_tempdir() -> io::Result<TempDir> {
    Builder::new()
        .permissions(Permissions::from_mode(0o700))
        .tempdir()
}

#[test]
fn private_tempdir_is_owner_only_regardless_of_umask() {
    let directory = private_tempdir().expect("owner-only temporary directory");
    let mode = directory
        .path()
        .metadata()
        .expect("temporary directory metadata")
        .permissions()
        .mode();

    assert_eq!(mode & 0o777, 0o700);
}
