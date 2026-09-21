//! Error contract for workspace automation.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

pub(crate) type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("[xtask/workspace] CARGO_MANIFEST_DIR is not set")]
    MissingManifestDir,
    #[error("[xtask/workspace] manifest directory has no workspace parent: {manifest_dir:?}")]
    MissingWorkspaceParent { manifest_dir: PathBuf },
    #[error("[xtask/command] failed to start `{command}`: {source}")]
    CommandStart { command: String, source: io::Error },
    #[error("[xtask/command] `{command}` failed with exit code {code:?}")]
    CommandFailed { command: String, code: Option<i32> },
    #[error("[xtask/rust_file_length_lint] failed to read directory `{path}`: {source}")]
    ReadDir { path: PathBuf, source: io::Error },
    #[error("[xtask/rust_file_length_lint] failed to read Rust file `{path}`: {source}")]
    ReadFile { path: PathBuf, source: io::Error },
    #[error("[xtask/rust_file_length_lint] found {count} violation(s):\n{details}")]
    FileLengthViolations { count: usize, details: String },
    #[error("[xtask/smoke] E2B configuration construction failed: {source}")]
    SmokeConfig {
        source: sandbox_e2b::E2bAdapterError,
    },
    #[error("[xtask/smoke] interface value or E2B backend construction failed: {source}")]
    SmokeBackend { source: sandbox_interface::Error },
    #[error("[xtask/review] failed to read `{path}`: {source}")]
    ReviewPromptRead { path: PathBuf, source: io::Error },
    #[error("[xtask/review] failed to start `{command}` during {stage}: {source}")]
    ReviewCommandStart {
        stage: &'static str,
        command: String,
        source: io::Error,
    },
    #[error("[xtask/review] `{command}` failed during {stage} with exit code {code:?}")]
    ReviewCommandFailed {
        stage: &'static str,
        command: String,
        code: Option<i32>,
    },
    #[error("[xtask/review] the worktree has tracked or untracked changes; commit them first")]
    ReviewDirtyWorktree,
    #[error("[xtask/review] the current branch has no upstream; push it with an upstream first")]
    ReviewMissingUpstream,
    #[error("[xtask/review] local HEAD does not match its upstream; push the current commit first")]
    ReviewUnpushedHead,
    #[error("[xtask/review] `{command}` returned non-UTF-8 output during {stage}")]
    ReviewNonUtf8 {
        stage: &'static str,
        command: String,
    },
    #[error("[xtask/review] the reviewer changed the worktree; inspect and restore it manually")]
    ReviewChangedWorktree,
}
