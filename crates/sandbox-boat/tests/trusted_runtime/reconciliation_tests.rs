//! Atomic revocation and exact committed-target ownership reconciliation.

use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    process::Command,
};

use super::fixture::{Attempt, Fixture, read_root_file};

#[test]
fn revoked_writer_cannot_commit_and_keeps_its_uncertain_fence() {
    let fixture = Fixture::new();
    let attempt = fixture.attempt(Attempt::write_manifest(&fixture, "file", b"payload"));
    attempt.stage("chunk-0", b"payload");
    assert_eq!(attempt.run("assemble").status.code(), Some(0));
    assert_eq!(attempt.run("reconcile").status.code(), Some(0));
    attempt.stage("chunk-0", b"payload");
    assert_eq!(attempt.run("assemble").status.code(), Some(0));
    assert_eq!(attempt.run("write").status.code(), Some(50));
    assert!(!fixture.destination("file").exists());
    let probe = fixture.workload(&format!("import os\ntry:\n os.unlink({:?})\nexcept PermissionError:\n raise SystemExit(0)\nraise SystemExit(8)", fixture.state.join("fences").join(&attempt.id)));
    assert_eq!(probe.status.code(), Some(0));
    let marker = Command::new("sudo")
        .args(["-n", "readlink"])
        .arg(fixture.state.join("fences").join(&attempt.id))
        .output()
        .unwrap();
    assert_eq!(marker.stdout, b"revoked\n");
}

#[test]
fn interrupted_handoff_reconciles_exact_bytes_and_preserves_visible_mode() {
    let fixture = Fixture::new();
    let attempt = fixture.attempt(Attempt::write_manifest(&fixture, "file", b"payload"));
    assert!(
        Command::new("sudo")
            .args(["-n", "touch"])
            .arg(fixture.destination("file"))
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("sudo")
            .args(["-n", "chmod", "644"])
            .arg(fixture.destination("file"))
            .status()
            .unwrap()
            .success()
    );
    let prepare =
        super::fixture::root_python("import sys;open(sys.argv[1],'wb').write(b'payload')")
            .arg(fixture.destination("file"))
            .output()
            .unwrap();
    assert_eq!(prepare.status.code(), Some(0));
    assert_eq!(attempt.run("reconcile").status.code(), Some(51));
    let info = fs::metadata(fixture.destination("file")).unwrap();
    assert_eq!(info.uid(), fixture.account.uid);
    assert_eq!(info.permissions().mode() & 0o777, 0o644);
    assert_eq!(read_root_file(&fixture.destination("file")), b"payload");
}
