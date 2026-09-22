//! Command-line parsing for workspace automation.

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Sandbox workspace automation tasks")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub(crate) enum Command {
    /// Run the complete credential-free verification sequence.
    Check,
    /// Run an AI review against origin/main after the branch is pushed.
    Review,
    /// Audit Rust source files for the repository line-count cap.
    RustFileLengthLint {
        /// Audit every Rust file under crates/ and xtask/.
        #[arg(long)]
        all: bool,
    },
    /// Construct the interface and E2B adapter without provider requests.
    SmokeTest,
}
