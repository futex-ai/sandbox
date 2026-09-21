use std::fs;
use std::path::Path;

use tempfile::TempDir;

use crate::file_length;

#[test]
fn accepts_rust_file_at_line_limit() {
    let workspace = test_workspace();
    let source = workspace.path().join("crates/demo/src/lib.rs");
    write_lines(&source, 300);

    file_length::run(workspace.path(), true).unwrap();
}

#[test]
fn rejects_rust_file_over_line_limit() {
    let workspace = test_workspace();
    let source = workspace.path().join("xtask/src/main.rs");
    write_lines(&source, 301);

    let error = file_length::run(workspace.path(), true)
        .unwrap_err()
        .to_string();
    assert!(error.contains("xtask/src/main.rs has 301 lines"));
}

fn test_workspace() -> TempDir {
    let workspace = TempDir::new().unwrap();
    fs::create_dir_all(workspace.path().join("crates/demo/src")).unwrap();
    fs::create_dir_all(workspace.path().join("xtask/src")).unwrap();
    workspace
}

fn write_lines(path: &Path, count: usize) {
    fs::write(path, "line\n".repeat(count)).unwrap();
}
