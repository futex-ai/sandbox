use std::ffi::OsString;

use clap::Parser;

use crate::check;
use crate::cli::{Cli, Command};
use crate::workspace::workspace_root_from;

#[test]
fn parses_every_command() {
    assert_eq!(
        Cli::try_parse_from(["xtask", "check"]).unwrap().command,
        Command::Check
    );
    assert_eq!(
        Cli::try_parse_from(["xtask", "review"]).unwrap().command,
        Command::Review
    );
    assert_eq!(
        Cli::try_parse_from(["xtask", "rust-file-length-lint", "--all"])
            .unwrap()
            .command,
        Command::RustFileLengthLint { all: true }
    );
    assert_eq!(
        Cli::try_parse_from(["xtask", "smoke-test"])
            .unwrap()
            .command,
        Command::SmokeTest
    );
}

#[test]
fn check_plan_uses_locked_deterministic_commands() {
    let commands = check::commands();
    assert_eq!(commands.len(), 4);
    assert_eq!(
        commands[0].args,
        ["metadata", "--locked", "--format-version", "1", "--no-deps"]
    );
    assert!(commands[2].args.contains(&"-D"));
    assert!(commands[2].args.contains(&"warnings"));
    assert!(commands[3].args.contains(&"--all-features"));
}

#[test]
fn workspace_discovery_uses_manifest_parent() {
    assert_eq!(
        workspace_root_from(Some(OsString::from("/repo/xtask"))).unwrap(),
        std::path::PathBuf::from("/repo")
    );
    assert!(workspace_root_from(None).is_err());
}
