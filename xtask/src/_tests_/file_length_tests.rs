use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::io;
use std::path::Path;

use tempfile::TempDir;

use crate::command::{CommandOutcome, CommandRunner, CommandSpec};
use crate::file_length;

type Response = io::Result<CommandOutcome>;

struct FakeRunner {
    responses: RefCell<VecDeque<Response>>,
    calls: RefCell<Vec<CommandSpec>>,
}

impl FakeRunner {
    fn new(responses: Vec<Response>) -> Self {
        Self {
            responses: RefCell::new(responses.into()),
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl CommandRunner for FakeRunner {
    fn execute(&self, spec: &CommandSpec) -> io::Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.clone());
        self.responses
            .borrow_mut()
            .pop_front()
            .expect("unexpected command")
    }
}

#[test]
fn accepts_rust_file_at_line_limit() {
    let workspace = test_workspace();
    let source = workspace.path().join("crates/demo/src/lib.rs");
    write_lines(&source, 300);

    file_length::run(workspace.path(), true, &FakeRunner::new(Vec::new())).unwrap();
}

#[test]
fn rejects_rust_file_over_line_limit() {
    let workspace = test_workspace();
    let source = workspace.path().join("xtask/src/main.rs");
    write_lines(&source, 301);

    let error = file_length::run(workspace.path(), true, &FakeRunner::new(Vec::new()))
        .unwrap_err()
        .to_string();
    assert!(error.contains("xtask/src/main.rs has 301 lines"));
}

#[test]
fn incremental_check_skips_unchanged_rust_files() {
    let workspace = test_workspace();
    let source = workspace.path().join("crates/demo/src/lib.rs");
    write_lines(&source, 301);

    let runner = FakeRunner::new(vec![ok(b""), ok(b"")]);
    file_length::run(workspace.path(), false, &runner).unwrap();

    let calls = runner.calls.borrow();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].args,
        [
            "diff",
            "--name-only",
            "-z",
            "--diff-filter=ACMR",
            "origin/main",
            "--",
            "crates",
            "xtask"
        ]
    );
    assert_eq!(
        calls[1].args,
        [
            "ls-files",
            "-z",
            "--others",
            "--exclude-standard",
            "--",
            "crates",
            "xtask"
        ]
    );
}

#[test]
fn incremental_check_includes_branch_and_untracked_rust_files() {
    let workspace = test_workspace();
    let tracked = workspace.path().join("crates/demo/src/tracked.rs");
    let untracked = workspace.path().join("xtask/src/untracked.rs");
    write_lines(&tracked, 301);
    write_lines(&untracked, 302);
    let runner = FakeRunner::new(vec![
        ok(b"crates/demo/src/tracked.rs\0"),
        ok(b"xtask/src/untracked.rs\0"),
    ]);

    let error = file_length::run(workspace.path(), false, &runner)
        .unwrap_err()
        .to_string();

    assert!(error.contains("crates/demo/src/tracked.rs has 301 lines"));
    assert!(error.contains("xtask/src/untracked.rs has 302 lines"));
}

fn ok(stdout: &[u8]) -> Response {
    Ok(CommandOutcome {
        success: true,
        code: Some(0),
        stdout: stdout.to_vec(),
    })
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
