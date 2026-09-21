# xtask

`xtask` is the repository's developer CLI for repeatable local and CI checks.
It is not a runtime dependency of either published library crate.

## Responsibilities

- Run the complete credential-free workspace verification sequence.
- Enforce the 300-line cap for Rust files under `crates/` and `xtask/`.
- Construct the interface and E2B adapter without provider requests.
- Delegate a manual, post-push branch review to the authenticated Codex CLI.

## What This Crate Does

`check` runs locked metadata, formatting, Clippy with warnings denied,
all-feature tests, file-length validation, and the smoke test. `review` fetches
`origin/main`, proves the worktree is clean and the current commit is pushed,
uses Codex's native base-branch reviewer, and verifies the worktree is still
unchanged afterward. The repository instructions apply the reporting contract
in `docs/implementation-review-prompt.md` to that reviewer.

External commands cross one injected runner boundary. Unit tests use a fake
runner, so they do not contact Git remotes or invoke Codex.

## Quick Start

```bash
cargo xtask check
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

- [Implementation review prompt](../docs/implementation-review-prompt.md)
- [Workspace README](../README.md)
- [Implementation plans](../plans/README.md)
