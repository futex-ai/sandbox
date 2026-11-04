//! Descriptor races and isolated source identity at the real Linux boundary.

use std::{fs, process::Command};

use serde_json::{Value, json};

use super::fixture::{Attempt, Fixture, root_python};

#[test]
fn a_replaced_visible_parent_cannot_redirect_a_bound_atomic_write() {
    let fixture = Fixture::new();
    let attempt = fixture.attempt(Attempt::write_manifest(&fixture, "parent/file", b"exact"));
    let parent = fixture.destination("parent");
    let moved = fixture.destination("moved");
    let outside = fixture.temporary.path().join("outside");
    let setup = root_python("import os,sys;os.mkdir(sys.argv[1]);os.mkdir(sys.argv[2]);open(sys.argv[2]+'/file','wb').write(b'outside')")
        .args([&parent, &outside]).output().unwrap();
    assert_eq!(setup.status.code(), Some(0));
    attempt.stage("chunk-0", b"exact");
    assert_eq!(attempt.run("assemble").status.code(), Some(0));
    let wrapper = format!(
        "import os\noriginal=os.replace\ndef replace(*args,**kwargs):\n os.rename({parent:?},{moved:?})\n os.symlink({outside:?},{parent:?})\n return original(*args,**kwargs)\nos.replace=replace\nexec({loader:?})",
        loader = include_str!("../../src/trusted_runtime/helpers/loader.py")
    );
    let output = root_python(&wrapper)
        .args([
            fixture.home.to_str().unwrap(),
            fixture.state.to_str().unwrap(),
            &fixture.controller,
            &attempt.id,
            &attempt.digest,
            "write",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{:?}", output.stderr);
    assert_eq!(fs::read(moved.join("file")).unwrap(), b"exact");
    assert_eq!(fs::read(outside.join("file")).unwrap(), b"outside");
}

#[test]
fn a_leaf_replaced_during_pread_keeps_the_open_file_and_original_size() {
    let fixture = Fixture::new();
    let attempt = fixture.attempt(
        json!({"operation":"read","root":fixture.workspace,"path":"file","offset":2,"max_bytes":3}),
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
    let wrapper = format!(
        "import os\noriginal=os.pread\ndef pread(*args):\n os.rename({file:?},{old:?})\n os.symlink('/etc/passwd',{file:?})\n return original(*args)\nos.pread=pread\nexec({loader:?})",
        file = fixture.destination("file"),
        old = fixture.destination("old"),
        loader = include_str!("../../src/trusted_runtime/helpers/loader.py")
    );
    let output = root_python(&wrapper)
        .args([
            fixture.home.to_str().unwrap(),
            fixture.state.to_str().unwrap(),
            &fixture.controller,
            &attempt.id,
            &attempt.digest,
            "read",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{:?}", output.stderr);
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["total_size"], 10);
    assert_eq!(
        fs::read(
            fixture
                .home
                .join(".sandbox-boat")
                .join(&attempt.id)
                .join("chunk-0")
        )
        .unwrap(),
        b"234"
    );
}

#[test]
fn changed_bundle_and_hostile_python_startup_cannot_execute_untrusted_source() {
    let fixture = Fixture::new();
    let attempt = fixture.attempt(Attempt::write_manifest(&fixture, "file", b"exact"));
    let sentinel = fixture.destination("forged");
    let setup = fixture.workload(&format!(
        "open({:?},'w').write({:?})",
        fixture.destination("json.py"),
        format!("open({sentinel:?},'w').write('forged')")
    ));
    assert_eq!(setup.status.code(), Some(0));
    let output = attempt
        .command("release")
        .current_dir(&fixture.workspace)
        .env("PYTHONPATH", &fixture.workspace)
        .env("PYTHONSTARTUP", fixture.destination("json.py"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(!sentinel.exists());
    let bundle = fixture
        .home
        .join(".sandbox-boat")
        .join(&attempt.id)
        .join("bundle.json");
    let mut content: Value = serde_json::from_slice(&fs::read(&bundle).unwrap()).unwrap();
    content["manifest"]["path"] = json!("evil");
    fs::write(bundle, serde_json::to_vec(&content).unwrap()).unwrap();
    assert_eq!(attempt.run("release").status.code(), Some(49));
    assert!(!sentinel.exists());
    assert!(
        Command::new("sudo")
            .args(["-n", "test", "-f"])
            .arg(attempt.state_path("release"))
            .status()
            .unwrap()
            .success()
    );
}
