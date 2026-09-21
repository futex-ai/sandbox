use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::io;

use tempfile::TempDir;

use crate::command::{CommandOutcome, CommandRunner, CommandSpec, OutputMode};
use crate::review;

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
fn invokes_exact_review_plan_with_prompt_on_stdin() {
    let workspace = workspace();
    let runner = FakeRunner::new(successful_run(b"review finding\n"));
    review::run(workspace.path(), &runner).unwrap();

    let calls = runner.calls.borrow();
    assert_eq!(calls.len(), 7);
    assert_eq!(
        calls[0].args,
        [
            "fetch",
            "--no-tags",
            "origin",
            "+refs/heads/main:refs/remotes/origin/main"
        ]
    );
    let review = &calls[5];
    assert_eq!(review.program, "codex");
    assert_eq!(
        review.args,
        [
            "exec",
            "review",
            "--base",
            "origin/main",
            "--ephemeral",
            "-"
        ]
    );
    assert_eq!(
        review.stdin.as_deref(),
        Some(b"review contract\n".as_slice())
    );
    assert_eq!(review.output, OutputMode::Inherit);
    assert!(calls.iter().all(|call| call.cwd == workspace.path()));
}

#[test]
fn accepts_successful_review_with_no_findings() {
    let workspace = workspace();
    review::run(workspace.path(), &FakeRunner::new(successful_run(b""))).unwrap();
}

#[test]
fn reports_fetch_failure() {
    assert_error(vec![failed()], "base branch fetch");
}

#[test]
fn rejects_dirty_or_untracked_worktree() {
    assert_error(vec![ok(b""), ok(b"?? new-file\n")], "commit them first");
}

#[test]
fn rejects_missing_upstream() {
    assert_error(vec![ok(b""), ok(b""), failed()], "no upstream");
}

#[test]
fn rejects_unpushed_head() {
    assert_error(
        vec![
            ok(b""),
            ok(b""),
            ok(b"origin/topic\n"),
            ok(b"local\n"),
            ok(b"remote\n"),
        ],
        "does not match",
    );
}

#[test]
fn reports_missing_codex_after_confirming_integrity() {
    let mut responses = pre_review();
    responses.push(Err(io::Error::new(io::ErrorKind::NotFound, "missing")));
    responses.push(ok(b""));
    assert_error(responses, "failed to start `codex exec review");
}

#[test]
fn reports_reviewer_failure_after_confirming_integrity() {
    let mut responses = pre_review();
    responses.extend([failed(), ok(b"")]);
    assert_error(responses, "Codex review");
}

#[test]
fn rejects_a_reviewer_that_changes_the_worktree() {
    let mut responses = pre_review();
    responses.extend([ok(b""), ok(b" M src/lib.rs\n")]);
    assert_error(responses, "changed the worktree");
}

fn assert_error(responses: Vec<Response>, expected: &str) {
    let workspace = workspace();
    let error = review::run(workspace.path(), &FakeRunner::new(responses))
        .unwrap_err()
        .to_string();
    assert!(error.contains(expected), "unexpected error: {error}");
}

fn successful_run(review_output: &[u8]) -> Vec<Response> {
    let mut responses = pre_review();
    responses.extend([ok(review_output), ok(b"")]);
    responses
}

fn pre_review() -> Vec<Response> {
    vec![
        ok(b""),
        ok(b""),
        ok(b"origin/topic\n"),
        ok(b"same\n"),
        ok(b"same\n"),
    ]
}

fn ok(stdout: &[u8]) -> Response {
    Ok(CommandOutcome {
        success: true,
        code: Some(0),
        stdout: stdout.to_vec(),
    })
}

fn failed() -> Response {
    Ok(CommandOutcome {
        success: false,
        code: Some(1),
        stdout: Vec::new(),
    })
}

fn workspace() -> TempDir {
    let workspace = TempDir::new().unwrap();
    fs::create_dir(workspace.path().join("docs")).unwrap();
    fs::write(
        workspace
            .path()
            .join("docs/implementation-review-prompt.md"),
        "review contract\n",
    )
    .unwrap();
    workspace
}
