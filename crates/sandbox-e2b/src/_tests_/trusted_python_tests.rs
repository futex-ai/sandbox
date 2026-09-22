//! Hostile Python startup isolation regressions.

use std::{fs, process::Command};

use tempfile::tempdir;

use super::{EXECUTABLE, command_args};

#[test]
fn trusted_helper_ignores_working_directory_and_python_path_modules() {
    let hostile = tempdir().expect("hostile Python module directory");
    fs::write(hostile.path().join("hashlib.py"), "raise SystemExit(91)\n")
        .expect("hostile standard-library shadow");
    fs::write(
        hostile.path().join("sitecustomize.py"),
        "raise SystemExit(92)\n",
    )
    .expect("hostile startup customization");
    let output = Command::new(EXECUTABLE)
        .args(command_args(
            "import hashlib; print(hashlib.sha256(b'sandbox').hexdigest())",
            std::iter::empty(),
        ))
        .current_dir(hostile.path())
        .env("PYTHONPATH", hostile.path())
        .output()
        .expect("run trusted Python probe");

    assert!(
        output.status.success(),
        "user-controlled Python startup ran: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let digest = String::from_utf8(output.stdout).expect("UTF-8 digest");
    assert_eq!(digest.trim().len(), 64);
    assert!(digest.trim().bytes().all(|byte| byte.is_ascii_hexdigit()));
}
