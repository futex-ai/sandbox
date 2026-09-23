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
Sandbox create requests carry their runtime-or-browser consumer class and typed
lifetime through provider metadata. Interactive sandboxes keep idle auto-pause;
bounded one-shot sandboxes remain running until explicit destroy or their
maximum 3600-second provider timeout and are never resumed. Managed inventory
returns recognized metadata and leaves missing or malformed values unknown.
Creates can also select a typed deny-by-default egress allowlist of canonical
IP, CIDR, and DNS destinations. E2B supports the IP and CIDR forms, rejects DNS
policies before provider access, preserves private and deployment deny ranges,
and refuses to recover a sandbox created with a different policy. Its public
control client independently revalidates typed allow destinations before
transport.
Image consumers also own the durable phase transitions: source creation,
one-shot preparation, snapshot dispatch, recover-only retries, and final source
cleanup are separate backend calls so eventual consistency cannot silently
duplicate provider work. A source remains paused while snapshot recovery finds
no completed result or more than one candidate; it reconnects only after the
adapter proves exactly one completed snapshot. Both snapshot probes in the
shared conformance harness accept immediate or asynchronous completion. The
harness retains every sandbox and snapshot create request before dispatch,
proves each returned identity through bounded one-second-paced recovery, and
runs its cwd, environment, and split-output check through one self-contained
`/bin/sh` command available in normal backend images. Cleanup always attempts
every tracked terminal, snapshot, and sandbox; an operation error remains the
reported error even if a cleanup step also fails. Trusted adapter helpers
isolate their interpreter startup from sandbox-owned modules and Python
environment customization.
Authenticated control routes reject dot-segment provider identifiers before
dispatch, and concrete control clients validate their HTTPS origin and
credentials before construction. Sandbox creation and inventory require a
lowercase DNS-label ID that every envd route can use. Snapshot creation and
inventory reject IDs that later control routes cannot use, and snapshot
inspection verifies that the provider returned the requested ID. Accepted
creates remain delivery-ambiguous, while malformed inventory remains
retryable. Snapshot operations also require one nonempty source and correlation
value. Sandbox create and ordinary connect responses require nonblank process
and private-traffic credentials; one-shot reconnect uses GET-only envd access
so it cannot refresh the destruction deadline. Provider mutations accept only
their exact acknowledgment, including an exact versioned screen-resize object;
process start and inventory responses must contain a nonzero PID. Connect streaming
collectors keep reading after a process end until they validate the required
success trailer; a missing, malformed, or unsuccessful trailer cannot look like
ordinary completion. Envd and private-port hosts always use the configured
routing domain, never a domain supplied by an injected control response.
Missing read credentials stay retryable.
Sandbox creation sends its one provider mutation without an inventory
preflight; ambiguous delivery can only use recovery reads. Terminal supervisors
persist a root-owned versioned PID, operation, terminal, and tag record before
the login shell can exit, allowing recovery and inspection to return the same
provider reference as `Exited`. Input rejects exited terminals, while close and
restored cleanup remain idempotent. Close retains trusted identity for final
output reads; restored cleanup removes it. Record-aware inspection and output
validate every present identity record, use selector-only fallback for
record-free live legacy terminals, and keep the retained transcript available
after unrelated PID reuse. Typed provider absence remains an error, and an
absolute helper deadline reserves identity-helper termination and return time.
Caller-controlled process durations are capped before provider access,
including 30-second terminal output waits and 300-second process operations.
Terminal creation and recovery also reject transcript limits above the shared
256 MiB readable-file ceiling before provider access. Stateless commands reject
an empty executable, more than 128 KiB of argv, or more than 64 MiB of combined
output before credentials are acquired. Trusted direct process calls may select
a validated absolute working directory and bounded environment map, while
template-owned resolution variables remain protected. Process and terminal
diagnostics expose selected metadata only; command text, environment entries,
and output contents are omitted. Terminal output, saved transcripts, and
returned stdout/stderr remain unmasked. Image failures report exit and capture
facts without output snippets. Image preparation validates every input path
and size before it connects, incomplete filesystem-size measurements fail closed, and cache
cleanup cannot follow setup-created symlink parents. Trusted provider helpers
keep uncertain-write fences and terminal logs in root-owned storage. A
replacement payload remains inside a root-owned private directory on the
destination filesystem until it is renamed through held descriptors. Recovery
reapplies the validated non-root workload owner, preserves the visible file
mode, and syncs the file and directory before success. Attempts with no possible
live writer discard their resolved fence, while uncertain writers keep it. The
interactive shell runs as an explicitly configured non-root account.

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
checks without provider secrets. Without `--all`, the file-length command
checks branch, staged, unstaged, and untracked Rust files; `cargo xtask check`
and the explicit `--all` form audit the complete workspace.

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
cleanup even after an operation fails. Every sandbox and snapshot request is
stored in cleanup state before dispatch; uncertain or eventually consistent
creation uses recover-only polling, and a returned sandbox is tracked before
later initialization can fail.

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
- [`docs/process-diagnostics.md`](docs/process-diagnostics.md) separates exact
  sensitive process data from metadata-only diagnostics.
- [`docs/juno-adoption.md`](docs/juno-adoption.md) describes the separate
  consumer cutover.
- [`plans/README.md`](plans/README.md) indexes implementation plans.
