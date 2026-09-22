# Stream Process Runs

## Summary

Add provider-neutral direct process streaming and implement it for E2B without
changing the existing collect-then-return process API. Consumers will be able
to observe stdout and stderr while a command runs for up to one hour, while
still receiving one typed terminal outcome for completion, overflow, idle or
absolute timeout, and transport failure.

The existing 300-second limit remains unchanged for collected process runs,
read-only execution, file transfer, terminal helpers, and every other existing
operation. Environment variables, working directories, network allowlists,
and sandbox creation options remain outside this change.

## Milestone 1: Define And Implement Streaming Execution

At the end of this milestone, both the provider-neutral interface and the E2B
adapter support bounded direct-argv streaming with one-hour absolute deadlines,
output-driven idle timeouts, verified Connect trailers, and best-effort process
cleanup.

- [x] Add provider-neutral stream requests, event and outcome enums, a boxed
      stream alias, and the 3,600-second maximum deadline.
- [x] Add `stream_process` to `SandboxBackend`, `SandboxService`, their unimock
      surfaces, and every backend implementation in this workspace.
- [x] Validate command bytes, per-stream limits, deadline, and idle timeout
      before any provider access while preserving every existing `run_process`
      bound and behavior.
- [x] Extend the injected E2B process transport with a streaming operation that
      emits decoded frames instead of collecting them.
- [x] Enforce one terminal outcome, reset idle time only for stdout or stderr
      data, require a success trailer after process end, and kill best-effort
      after every unfinished termination path including consumer drop.
- [x] Give only the new streaming HTTP request a timeout derived from its
      requested deadline plus a small transport allowance.
- [x] Add focused regressions through injected process and Connect HTTP
      transports for validation, event ordering, keep-alive idle expiry,
      absolute deadline expiry, overflow, missing trailers, transport failure,
      request timeout derivation, and consumer-drop cleanup.

## Milestone 2: Prove Conformance And Document The Contract

At the end of this milestone, the shared harness proves streaming behavior with
portable `/bin/sh` commands and every public document explains the new boundary
without implying that existing 300-second operations changed.

- [x] Add a `/bin/sh` streaming probe to the shared backend conformance harness
      and update the alternate fake backend and E2B conformance mocks.
- [x] Update `docs/sandbox-contract.md` and `docs/e2b-adapter.md` with event
      ordering, terminal outcomes, validation, timer, trailer, and cleanup
      semantics.
- [x] Update the workspace README and both crate READMEs with the streaming API,
      one-hour ceiling, and unchanged 300-second legacy ceilings.
- [x] Review the complete diff for documentation consistency, file length,
      unrelated changes, and accidental expansion into out-of-scope options.

## Milestone 3: Validate, Commit, Push, And Review

At the end of this milestone, the implementation is fully tested, committed,
pushed, and independently reviewed against `origin/main`.

- [x] Run focused interface and E2B streaming tests, including a local
      construction or transport smoke test.
- [x] Run `cargo fmt --all -- --check`, Clippy, the complete workspace test
      suite, the Rust file-length lint, and `cargo xtask check`; fix every
      failure until all checks pass.
- [x] Audit tracked files for secrets, generated artifacts, whitespace errors,
      and unrelated edits.
- [ ] Run `git add -A`, commit all completed work with a Conventional Commit,
      and push the current branch with every new file tracked.
- [ ] Run `cargo xtask review` after the push so the AI reviewer checks the
      clean local diff against `origin/main`; record and report every finding
      without automatically fixing it.
- [ ] Mark the plan complete and move it from Active to Completed in
      `plans/README.md` after the implementation and review workflow finishes.
