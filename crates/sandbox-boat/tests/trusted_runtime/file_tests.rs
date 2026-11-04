//! Exact file bytes, confinement, integrity, and controller/workload separation.

use std::{fs, os::unix::fs::MetadataExt, process::Command};

use serde_json::{Value, json};

use super::fixture::{Attempt, Fixture};

#[test]
fn atomic_replace_hands_owner_and_mode_to_an_isolated_workload() {
    let fixture = Fixture::new();
    let bytes = b"new\0binary\xffcontents";
    let attempt = fixture.attempt(Attempt::write_manifest(&fixture, "file", bytes));
    let initial = fixture.workload(&format!(
        "open({:?}, 'wb').write(b'old')",
        fixture.destination("file")
    ));
    assert_eq!(initial.status.code(), Some(0));
    assert!(
        Command::new("sudo")
            .args(["-n", "chmod", "640"])
            .arg(fixture.destination("file"))
            .status()
            .unwrap()
            .success()
    );
    attempt.stage("chunk-0", bytes);
    assert_eq!(attempt.run("assemble").status.code(), Some(0));
    assert_eq!(attempt.run("write").status.code(), Some(0));
    assert_eq!(
        fixture
            .workload(&format!(
                "import sys;sys.stdout.buffer.write(open({:?}, 'rb').read())",
                fixture.destination("file")
            ))
            .stdout,
        bytes
    );
    let metadata = fs::metadata(fixture.destination("file")).unwrap();
    assert_eq!(metadata.uid(), 65534);
    assert_eq!(metadata.mode() & 0o777, 0o640);
    let probe = fixture.workload(&format!("import os\np={:?}\nopen(p+'/own', 'wb').write(b'allowed')\nfor p in [{:?}, {:?}]:\n try:\n  open(p+'/forged', 'wb').write(b'forged')\n except PermissionError:\n  continue\n raise SystemExit(7)", fixture.workspace, fixture.home, fixture.state));
    assert_eq!(probe.status.code(), Some(0));
    assert_eq!(fs::read(fixture.destination("own")).unwrap(), b"allowed");
    assert_eq!(attempt.run("clean").status.code(), Some(0));
    assert!(!attempt.state_path("payload").exists());
    assert_eq!(
        fs::read_dir(fixture.home.join(".sandbox-boat"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn descriptor_read_returns_selected_bytes_full_size_and_rejects_nonregular_paths() {
    let fixture = Fixture::new();
    let attempt = fixture.attempt(
        json!({"operation":"read","root":fixture.workspace,"path":"file","offset":3,"max_bytes":4}),
    );
    assert_eq!(
        fixture
            .workload(&format!(
                "open({:?},'wb').write(b'0123456789')",
                fixture.destination("file")
            ))
            .status
            .code(),
        Some(0)
    );
    let output = attempt.run("read");
    assert_eq!(output.status.code(), Some(0), "{:?}", output.stderr);
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["total_size"], 10);
    assert_eq!(result["sizes"], json!([4]));
    assert_eq!(
        fs::read(
            fixture
                .home
                .join(".sandbox-boat")
                .join(&attempt.id)
                .join("chunk-0")
        )
        .unwrap(),
        b"3456"
    );
    assert_eq!(
        super::fixture::root_python("import os,sys;os.symlink('/etc/passwd',sys.argv[1])")
            .arg(fixture.destination("link"))
            .output()
            .unwrap()
            .status
            .code(),
        Some(0)
    );
    let linked = fixture.attempt(json!({"operation":"read","root":fixture.workspace,"path":"link","offset":0,"max_bytes":100}));
    assert_eq!(linked.run("read").status.code(), Some(46));
    assert_eq!(
        super::fixture::root_python("import os,sys;os.mkdir(sys.argv[1])")
            .arg(fixture.destination("directory"))
            .output()
            .unwrap()
            .status
            .code(),
        Some(0)
    );
    let directory = fixture.attempt(json!({"operation":"read","root":fixture.workspace,"path":"directory","offset":0,"max_bytes":100}));
    assert_eq!(directory.run("read").status.code(), Some(46));
    assert_eq!(
        super::fixture::root_python("import os,sys;os.symlink('/etc',sys.argv[1])")
            .arg(fixture.destination("parent"))
            .output()
            .unwrap()
            .status
            .code(),
        Some(0)
    );
    let parent = fixture.attempt(json!({"operation":"read","root":fixture.workspace,"path":"parent/passwd","offset":0,"max_bytes":100}));
    assert_eq!(parent.run("read").status.code(), Some(46));
}

#[test]
fn same_size_staging_tamper_cannot_reach_the_atomic_writer() {
    let fixture = Fixture::new();
    let attempt = fixture.attempt(Attempt::write_manifest(&fixture, "file", b"good"));
    attempt.stage("chunk-0", b"evil");
    assert_eq!(attempt.run("assemble").status.code(), Some(51));
    assert!(!fixture.destination("file").exists());
    assert_eq!(attempt.run("reconcile-resolved").status.code(), Some(0));
    assert_eq!(attempt.run("clean").status.code(), Some(0));
    let fences = Command::new("sudo")
        .args(["-n", "find"])
        .arg(fixture.state.join("fences"))
        .args(["-mindepth", "1", "-print"])
        .output()
        .unwrap();
    assert!(fences.status.success());
    assert_eq!(fences.stdout, b"");
}
