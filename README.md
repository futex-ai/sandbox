# Sandbox

Reusable Rust contracts for remote development sandboxes and provider
adapters. The workspace keeps application code dependent on a provider-neutral
interface while composition roots choose a concrete provider.

```text
application -> sandbox-interface <- sandbox-e2b
```

## Workspace

| Crate | Purpose |
| --- | --- |
| [`sandbox-interface`](crates/sandbox-interface/README.md) | Stable IDs, data types, errors, service/backend traits, mocks, and the backend conformance harness. |
| [`sandbox-e2b`](crates/sandbox-e2b/README.md) | The first concrete backend, implemented with E2B control and envd APIs. |
| [`xtask`](xtask/README.md) | Repeatable formatting, linting, test, smoke, file-length, and review commands. |

Consumers should depend on `sandbox-interface`. Only the process that selects
and constructs providers should also depend on `sandbox-e2b`. This keeps E2B
credentials, payloads, access tokens, and errors out of higher-level services.

## Developer Setup

The repository pins Rust 1.95.0 and installs `rustfmt` and Clippy through
`rust-toolchain.toml`. From the repository root, run:

```bash
cargo xtask check
```

That command performs locked metadata validation, formatting, workspace
Clippy with warnings denied, all-feature tests, the 300-line Rust source audit,
and a credential-free construction smoke test. Individual commands are:

```bash
cargo metadata --locked --format-version 1 --no-deps
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo xtask rust-file-length-lint --all
cargo xtask smoke-test
```

None of these commands contacts E2B. GitHub Actions runs the same deterministic
checks without provider secrets.

## Optional Live E2B Tests

Live tests are feature-gated, ignored by default, and may incur provider cost.
Run them only when external calls are intended:

```bash
E2B_API_KEY=... cargo test -p sandbox-e2b \
  --features live-e2b --test live_e2b -- --ignored

E2B_API_KEY=... E2B_SCREEN_TEMPLATE_ID=... \
  cargo test -p sandbox-e2b --features live-e2b --test live_e2b \
  live_e2b_private_screen_bridges -- --ignored
```

`E2B_TEMPLATE_ID` selects a lifecycle-test template; it defaults to E2B's
`base` template. `E2B_SCREEN_TEMPLATE_ID` must identify a compatible template
published by the separately owned template release project. The tests attempt
cleanup even after an operation fails.

## Review Workflow

Install and authenticate the Codex CLI locally before running the manual AI
review. After all checks pass, commit every change and push the current branch,
then run:

```bash
cargo xtask review
```

The command fetches `origin/main`, requires a clean worktree and a pushed
upstream commit, and delegates the complete branch diff to `codex exec review`.
It never runs in CI and verifies that the reviewer did not alter the worktree.
Review findings must be explained in simple language for a reader with no
prior repository context; they are reported for a maintainer to decide on and
are not changed automatically.

## Provenance

The two library crates were extracted from Juno commit
`64e6dd1c1fcc0d44c73fa1b31879cb4ba80448c6`. Workspace layout and automation
were adapted from the standalone AI shared-library repository at commit
`35da3dd9f5b7316bcfbdfa99c665e406220ffaec`.

The extraction copied provider-neutral behavior, conformance coverage, the E2B
transport boundary, and safe opt-in live tests. It adapted package names,
documentation, diagnostics, fixtures, and deployment conventions; it did not
copy application orchestration or provider template infrastructure.

## Key Code And Docs

- [`sandbox-interface/src/backend.rs`](crates/sandbox-interface/src/backend.rs)
  defines provider obligations.
- [`sandbox-e2b/src/backend/configured.rs`](crates/sandbox-e2b/src/backend/configured.rs)
  dispatches those obligations to E2B.
- [`docs/sandbox-contract.md`](docs/sandbox-contract.md) documents the shared
  protocol.
- [`docs/e2b-adapter.md`](docs/e2b-adapter.md) documents adapter guarantees and
  configuration.
- [`docs/juno-adoption.md`](docs/juno-adoption.md) describes the separate
  consumer cutover.
- [`plans/README.md`](plans/README.md) indexes implementation plans.
