//! Full credential-free local verification plan.

use std::path::Path;

use crate::command::{self, CommandRunner};
use crate::error::Result;
use crate::{file_length, smoke};

pub(crate) fn run(root: &Path, runner: &dyn CommandRunner) -> Result<()> {
    for command in commands() {
        command::run(root, runner, command.program, command.args)?;
    }
    file_length::run(root, true, runner)?;
    smoke::run()
}

pub(crate) struct CheckCommand {
    pub(crate) program: &'static str,
    pub(crate) args: &'static [&'static str],
}

pub(crate) fn commands() -> Vec<CheckCommand> {
    vec![
        CheckCommand {
            program: "cargo",
            args: &["metadata", "--locked", "--format-version", "1", "--no-deps"],
        },
        CheckCommand {
            program: "cargo",
            args: &["fmt", "--all", "--", "--check"],
        },
        CheckCommand {
            program: "cargo",
            args: &[
                "clippy",
                "--locked",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
        },
        CheckCommand {
            program: "cargo",
            args: &["test", "--locked", "--workspace", "--all-features"],
        },
    ]
}
