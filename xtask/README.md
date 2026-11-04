# xtask

`xtask` is the repository's developer CLI for repeatable local and CI checks.
It is not a runtime dependency of the published library crates.

## Responsibilities

- Run the complete credential-free workspace verification sequence.
- Enforce the 300-line cap for Rust files under `crates/` and `xtask/`.
- Construct E2B and each independent capability object without provider requests.
- Construct Boat lifecycle, snapshot, process, file, and capacity objects without provider requests.
- Delegate a manual, post-push branch review to the authenticated Codex CLI.

## What This Crate Does

`check` runs locked metadata, formatting, Clippy with warnings denied,
all-feature tests, a full file-length validation, and the smoke test. The
standalone file-length command checks Rust files changed from `origin/main`,
including staged, unstaged, and untracked files; `--all` scans every Rust file
under `crates/` and `xtask/`. `review` fetches `origin/main`, proves the
worktree is clean and the current commit is pushed, uses Codex's native
base-branch reviewer, and verifies the worktree is still unchanged afterward.
`review` is an optional manual check. An agent's post-push review follows the
`implementation-review` skill of `futex-ai/skills`.

External commands cross one injected runner boundary. Unit tests use a fake
runner, so they do not contact Git remotes or invoke Codex.

## Quick Start

```bash
cargo xtask check
cargo xtask rust-file-length-lint
cargo xtask rust-file-length-lint --all
cargo xtask smoke-test
# After committing and pushing:
cargo xtask review
```

## Development

```bash
cargo test -p xtask
cargo clippy -p xtask --all-targets -- -D warnings
```

The review command requires an installed, authenticated Codex CLI. It is a
manual developer action and is deliberately excluded from CI.

### Key Code

- `src/check.rs` — ordered verification plan.
- `src/command.rs` — real and injectable command execution.
- `src/file_length.rs` — Rust source length audit.
- `src/smoke.rs` — offline construction boundary.
- `src/review.rs` — Git preflight and Codex delegation.

### Related Docs

- The `implementation-review` skill of
  [`futex-ai/skills`](https://github.com/futex-ai/skills) is the source of the
  review procedure and its prompt.
- [Workspace README](../README.md)
