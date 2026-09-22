//! Post-push Codex review orchestration with Git safety checks.

use std::path::Path;

use crate::command::{CommandOutcome, CommandRunner, CommandSpec};
use crate::error::{Error, Result};

pub(crate) fn run(root: &Path, runner: &dyn CommandRunner) -> Result<()> {
    execute_checked(
        runner,
        &CommandSpec::inherited(
            root,
            "git",
            &[
                "fetch",
                "--no-tags",
                "origin",
                "+refs/heads/main:refs/remotes/origin/main",
            ],
        ),
        "base branch fetch",
    )?;
    if !worktree_status(root, runner, "preflight worktree check")?.is_empty() {
        return Err(Error::ReviewDirtyWorktree);
    }
    let upstream = execute_allow_failure(
        runner,
        &CommandSpec::captured(
            root,
            "git",
            &[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ],
        ),
        "upstream lookup",
    )?;
    if !upstream.success || decode(&upstream, "git upstream lookup")?.trim().is_empty() {
        return Err(Error::ReviewMissingUpstream);
    }
    let head = revision(root, runner, "HEAD", "local revision lookup")?;
    let upstream = revision(root, runner, "@{upstream}", "upstream revision lookup")?;
    if head != upstream {
        return Err(Error::ReviewUnpushedHead);
    }

    let review = CommandSpec::inherited(
        root,
        "codex",
        &["exec", "review", "--base", "origin/main", "--ephemeral"],
    );
    let review_result = execute_checked(runner, &review, "Codex review");
    let changed = !worktree_status(root, runner, "post-review worktree check")?.is_empty();
    if changed {
        return Err(Error::ReviewChangedWorktree);
    }
    review_result.map(|_| ())
}

fn revision(
    root: &Path,
    runner: &dyn CommandRunner,
    reference: &str,
    stage: &'static str,
) -> Result<String> {
    let spec = CommandSpec::captured(root, "git", &["rev-parse", reference]);
    let output = execute_checked(runner, &spec, stage)?;
    Ok(decode(&output, &spec.label())?.trim().to_owned())
}

fn worktree_status(
    root: &Path,
    runner: &dyn CommandRunner,
    stage: &'static str,
) -> Result<Vec<u8>> {
    let spec = CommandSpec::captured(
        root,
        "git",
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );
    Ok(execute_checked(runner, &spec, stage)?.stdout)
}

fn execute_checked(
    runner: &dyn CommandRunner,
    spec: &CommandSpec,
    stage: &'static str,
) -> Result<CommandOutcome> {
    let outcome = execute_allow_failure(runner, spec, stage)?;
    if outcome.success {
        Ok(outcome)
    } else {
        Err(Error::ReviewCommandFailed {
            stage,
            command: spec.label(),
            code: outcome.code,
        })
    }
}

fn execute_allow_failure(
    runner: &dyn CommandRunner,
    spec: &CommandSpec,
    stage: &'static str,
) -> Result<CommandOutcome> {
    runner
        .execute(spec)
        .map_err(|source| Error::ReviewCommandStart {
            stage,
            command: spec.label(),
            source,
        })
}

fn decode<'a>(output: &'a CommandOutcome, command: &str) -> Result<&'a str> {
    std::str::from_utf8(&output.stdout).map_err(|_| Error::ReviewNonUtf8 {
        stage: "Git preflight",
        command: command.to_owned(),
    })
}

#[cfg(test)]
#[path = "_tests_/review_tests.rs"]
mod review_tests;
