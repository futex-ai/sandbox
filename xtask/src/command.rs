//! Injectable external-command execution.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OutputMode {
    Capture,
    Inherit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommandSpec {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) cwd: PathBuf,
    pub(crate) stdin: Option<Vec<u8>>,
    pub(crate) output: OutputMode,
}

impl CommandSpec {
    pub(crate) fn inherited(root: &Path, program: &str, args: &[&str]) -> Self {
        Self {
            program: program.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            cwd: root.to_path_buf(),
            stdin: None,
            output: OutputMode::Inherit,
        }
    }

    pub(crate) fn captured(root: &Path, program: &str, args: &[&str]) -> Self {
        Self {
            output: OutputMode::Capture,
            ..Self::inherited(root, program, args)
        }
    }

    pub(crate) fn label(&self) -> String {
        command_label(&self.program, &self.args)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommandOutcome {
    pub(crate) success: bool,
    pub(crate) code: Option<i32>,
    pub(crate) stdout: Vec<u8>,
}

pub(crate) trait CommandRunner {
    fn execute(&self, spec: &CommandSpec) -> io::Result<CommandOutcome>;
}

pub(crate) struct ProcessRunner;

impl CommandRunner for ProcessRunner {
    fn execute(&self, spec: &CommandSpec) -> io::Result<CommandOutcome> {
        let mut command = Command::new(&spec.program);
        command.args(&spec.args).current_dir(&spec.cwd);
        if spec.stdin.is_some() {
            command.stdin(Stdio::piped());
        }
        match spec.output {
            OutputMode::Capture => {
                command.stdout(Stdio::piped()).stderr(Stdio::inherit());
            }
            OutputMode::Inherit => {
                command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
            }
        }
        let mut child = command.spawn()?;
        if let Some(input) = &spec.stdin {
            child
                .stdin
                .take()
                .ok_or_else(|| io::Error::other("child stdin was not piped"))?
                .write_all(input)?;
        }
        let output = child.wait_with_output()?;
        Ok(CommandOutcome {
            success: output.status.success(),
            code: output.status.code(),
            stdout: output.stdout,
        })
    }
}

pub(crate) fn run(
    root: &Path,
    runner: &dyn CommandRunner,
    program: &str,
    args: &[&str],
) -> Result<()> {
    let spec = CommandSpec::inherited(root, program, args);
    let outcome = runner
        .execute(&spec)
        .map_err(|source| Error::CommandStart {
            command: spec.label(),
            source,
        })?;
    if outcome.success {
        Ok(())
    } else {
        Err(Error::CommandFailed {
            command: spec.label(),
            code: outcome.code,
        })
    }
}

pub(crate) fn command_label(program: &str, args: &[String]) -> String {
    let mut parts = vec![program.to_owned()];
    parts.extend(args.iter().cloned());
    parts.join(" ")
}
