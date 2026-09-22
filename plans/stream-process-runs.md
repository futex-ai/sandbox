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
- [x] Run `git add -A`, commit all completed work with a Conventional Commit,
      and push the current branch with every new file tracked.
- [x] Run `cargo xtask review` after the push so the AI reviewer checks the
      clean local diff against `origin/main`; record and report every finding
      without automatically fixing it.
- [x] Mark the plan complete and move it from Active to Completed in
      `plans/README.md` after the implementation and review workflow finishes.

## Review Outcome

The post-push review completed on commit `53d4811` and reported two findings.
They are intentionally recorded without changing the reviewed implementation
so the maintainer can choose the follow-up.

1. **Severity: medium — preserve abnormal process termination in events.** In
   `crates/sandbox-e2b/src/process/stream_run.rs`, the E2B `exited` flag is
   discarded when mapping a provider end frame to `Exited { exit_code }`. A
   signal termination can carry the default zero exit code, so doing nothing
   can make a killed command look successful to a consumer. Option A: extend
   the public exit event to preserve the provider's normal-exit flag. Option B:
   add a distinct signal-termination event. **Recommendation: A**, because it
   retains status already preserved by collected execution with the smallest
   semantic change.
2. **Severity: medium — deliver deadline outcomes before cleanup.** In
   `crates/sandbox-e2b/src/process/stream_run.rs`, the worker awaits the bounded
   best-effort kill before it sends `DeadlineExpired`. If the kill request
   stalls, doing nothing can delay the terminal outcome by up to three seconds
   beyond the advertised absolute deadline. Option A: send and close the event
   stream at the deadline, then perform cleanup independently. Option B: add
   and document a cleanup grace period beyond the public deadline.
   **Recommendation: A**, because it keeps the absolute deadline meaningful
   while retaining best-effort cleanup.

## Milestone 4: Resolve Review Findings

At the end of this milestone, exit events preserve normal-versus-signal
termination and terminal outcomes close the observable stream before bounded
cleanup can extend its deadline. Timer expiry after process end is classified
as incomplete transport until the required success trailer arrives.

- [x] Add failing regressions for a signalled process with a default zero exit
      code and for a deadline outcome whose process kill remains stalled.
- [x] Preserve E2B's normal-exit flag in the provider-neutral `Exited` event and
      update every conformance fake, mock, test, and contract example.
- [x] Deliver the one terminal outcome and close the event stream before
      awaiting best-effort cleanup, while retaining consumer-drop cleanup.
- [x] Update the contract, adapter documentation, crate READMEs, and workspace
      README with the corrected exit and deadline semantics.
- [x] Run focused regressions, `cargo fmt --all -- --check`, Clippy, the full
      workspace test suite, the file-length lint, smoke coverage, and
      `cargo xtask check`; fix every failure until all checks pass.
- [x] Audit the complete diff for secrets, artifacts, whitespace errors,
      unrelated edits, and consistency with the original streaming scope.
- [x] Run `git add -A`, commit all completed work with a Conventional Commit,
      and push the current branch with every new file tracked.
- [x] Run `cargo xtask review` after the push against `origin/main`; record and
      report every new finding without automatically fixing it.
- [x] Add a regression that fills the event queue, pauses the consumer, and
      proves cleanup starts before queued data and the outcome are drained.
- [x] Resolve the terminal-outcome backpressure finding below after the
      maintainer chooses a solution.
- [x] Re-run focused and full checks, then audit the backpressure fix diff.
- [x] Commit and push the backpressure fix, then run `cargo xtask review` on the
      clean branch and record every new finding without automatically fixing it.
- [x] Resolve the post-exit timer classification finding below after the
      maintainer chooses a solution.
- [x] Add failing regressions for deadline expiry after a delivered exit and
      idle expiry while exit-event delivery is blocked by a full queue.
- [x] Map both timer outcomes to transport failure after process end without
      changing their pre-exit meaning.
- [x] Update the contract, adapter guide, and workspace and crate READMEs with
      the post-exit trailer rule.
- [x] Re-run focused tests and `cargo xtask check`, then audit the complete
      review-finding fix.
- [ ] Commit and push the fix, then run `cargo xtask review` against
      `origin/main` and record every new finding without automatically fixing
      it.
- [ ] Mark this milestone complete and move the plan from Active to Completed
      in `plans/README.md` after the review workflow finishes.

### Follow-up Review Finding

1. **Severity: high — decouple cleanup from terminal-event backpressure.** In
   `crates/sandbox-e2b/src/process/stream_state.rs:102`, the terminal outcome is
   sent through the same bounded queue as stdout and stderr. If all 16 slots are
   full and the consumer retains the stream without polling it, that send waits
   indefinitely and cleanup never starts. Doing nothing can leave a timed-out,
   overflowing, or failed provider process running and can delay its terminal
   outcome arbitrarily beyond the public deadline. Option A: store or deliver
   the terminal outcome independently from the bounded data queue and start
   cleanup without waiting for queue capacity. Option B: discard queued data
   through an explicit consumer-lag outcome before cleanup. **Recommendation:
   A**, because it preserves already-emitted data ordering for consumers that
   resume polling while ensuring cleanup is never blocked by their backpressure.

   **Resolution:** Option A now uses an independent one-shot terminal slot. A
   regression fills the bounded queue, pauses the consumer, proves cleanup
   starts, then verifies queued output still precedes the outcome.

2. **Severity: medium — classify post-exit timer expiry as transport
   failure.** In `crates/sandbox-e2b/src/process/stream_run.rs:125`, the
   deadline and idle timer branches emit their timeout outcomes even after an
   `Exited` event has been observed but before the required success trailer.
   Doing nothing reports an unverified completion sequence as a routine timeout,
   while collected execution and the documented trailer contract treat a
   missing final marker as malformed transport completion. Option A: once exit
   is observed, map deadline or idle expiry—including expiry while delivering
   `Exited`—to `TransportFailure`. Option B: change the shared contract and
   collected behavior so timeout outcomes take precedence after exit.
   **Recommendation: A**, because it preserves the existing final-trailer
   invariant consistently across collected and streaming execution.

   **Resolution:** Option A now maps either timer to `TransportFailure` after
   process end, including while `Exited` is blocked on queue capacity. Focused
   regressions preserve `DeadlineExpired` and `IdleTimeout` before process end.
