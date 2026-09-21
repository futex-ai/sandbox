//! Workspace automation binary.

mod check;
mod cli;
mod command;
mod error;
mod file_length;
mod review;
mod smoke;
mod workspace;

use clap::Parser;

use crate::cli::{Cli, Command};
use crate::command::ProcessRunner;
use crate::error::Result;
use crate::workspace::workspace_root;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let root = workspace_root()?;
    let runner = ProcessRunner;

    match cli.command {
        Command::Check => check::run(&root, &runner),
        Command::Review => review::run(&root, &runner),
        Command::RustFileLengthLint { all } => file_length::run(&root, all),
        Command::SmokeTest => smoke::run(),
    }
}

#[cfg(test)]
#[path = "_tests_/main_tests.rs"]
mod main_tests;
