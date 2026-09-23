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
- [x] Commit and push the fix, then run `cargo xtask review` against
      `origin/main` and record every new finding without automatically fixing
      it.
- [x] Finish this review cycle and track its new findings in a separate
      milestone.

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

## Milestone 5: Preserve Sandbox Lifetime And Trailer Finality

At the end of this milestone, an accepted streaming deadline is covered by the
E2B sandbox lifetime, and a Connect end-stream trailer is the final frame in its
decoded stream.

- [x] Add failing regressions for stream deadlines longer than the configured
      sandbox timeout and for frames following a success trailer.
- [x] Add a typed control operation for a call-specific nonzero connection
      timeout while leaving every existing connection path unchanged.
- [x] Connect streaming calls with the greater of the configured sandbox
      timeout and the requested deadline rounded up to whole seconds.
- [x] Make the shared Connect decoder reject bytes after an end-stream frame
      and prove collected and incremental paths fail closed.
- [x] Update the contract, adapter guide, workspace README, and crate READMEs.
- [x] Run focused tests and `cargo xtask check`, then audit the complete diff.
- [x] Commit and push the fixes, then run `cargo xtask review` against
      `origin/main` and report every new finding without automatically fixing
      it.
- [x] Complete this milestone after reviewing `2bc577a`; track its three new
      findings in Milestone 6 before moving the plan to Completed.

### Second Follow-up Review Findings

1. **Severity: high — extend the E2B sandbox lifetime for long streams.** The
   streaming backend accepted deadlines up to one hour but connected with only
   the adapter's fixed sandbox timeout, which can be 600 seconds. A valid long
   command could therefore lose its sandbox before its deadline. Option A: use
   a call-specific connection timeout covering the stream deadline. Option B:
   reject deadlines longer than the configured timeout. **Recommendation: A**,
   because it preserves the advertised one-hour capability.

   **Resolution:** Option A now uses the greater of the configured timeout and
   the deadline rounded up to whole seconds. The concrete control client
   forwards and validates that call-specific value.

2. **Severity: medium — reject frames following a success trailer.** A valid
   success trailer followed by another frame in the same HTTP fragment was
   reported as `Completed` because consumers stopped at the trailer and ignored
   the remaining decoded frames. Option A: check only the streaming consumer's
   decoded batch. Option B: enforce terminal-frame finality in the shared
   decoder. **Recommendation: B**, so every Connect collector fails closed.

   **Resolution:** Option B makes the decoder terminal-aware and rejects any
   trailing or subsequently supplied bytes after an end-stream frame.

## Milestone 6: Finish Overflow, HTTP Finality, And Deadline Fixes

At the end of this milestone, overflow cleanup starts even with a full output
queue, successful collectors validate HTTP EOF after the trailer, and one
absolute execution budget includes E2B sandbox connection time.

- [x] Add failing regressions for a full queue with an overflowing final
      prefix, bytes in a later HTTP chunk, and connection latency.
- [x] Store the bounded overflow prefix alongside the independent terminal
      outcome, preserving output order without delaying cleanup.
- [x] Keep streaming, combined, and split collectors reading through HTTP EOF
      and reject later bytes, transport failure, or expiry before that EOF.
- [x] Anchor the absolute budget before control connection and pass the same
      origin to the process transport; skip process start if setup exhausts it.
- [x] Update the shared contract, adapter guide, and affected READMEs.
- [x] Run focused regressions, formatting, Clippy, workspace tests, file-length
      validation, smoke coverage, and `cargo xtask check`; audit the diff.
- [x] Run `git add -A`, commit with Conventional Commits, and push the branch.
- [x] Run `cargo xtask review` after the push against `origin/main`; report
      every finding without automatically fixing it.
- [x] Complete this review cycle; keep the plan Active for the new finding
      awaiting a maintainer decision in Milestone 7.

### Milestone 6 Review Outcome

The three requested fixes were committed and pushed in `c3c981a`.
`cargo xtask check` passed with 307 tests passing and three opt-in live tests
ignored. The post-push review completed without changing the worktree. Its own
test rerun encountered sandbox restrictions on opening local sockets; those
tests passed in the full workspace check.

1. **Severity: high — anchor idle resets to provider arrival.** At
   `crates/sandbox-e2b/src/process/stream_run.rs:212`, the streaming adapter
   resets the idle timer when it processes output. One HTTP chunk can contain
   more frames than the 16-slot output queue holds, so earlier events may wait
   for the consumer before later, already-arrived frames reset the timer.
   Doing nothing lets a slow consumer make old bytes count as fresh activity,
   postponing termination and cleanup until the absolute deadline. Option A:
   record output arrival time before waiting for queue capacity and use it for
   idle resets. Option B: add a consumer-lag outcome that ends a stream when
   its queue fills, changing the public contract and slow-consumer behavior.
   **Recommendation: A**, preserving queued output while keeping the idle
   budget independent of consumer speed. The maintainer chose A for Milestone 7.

## Milestone 7: Resolve Buffered-Output Idle Timing

Delayed consumption of output that has already arrived cannot refresh the
process idle deadline.

- [x] Confirm the chosen follow-up for the new review finding.
- [x] Add a regression with a coalesced output batch, a full queue, and a slow
      consumer; verify that old output cannot postpone idle expiry or cleanup.
- [x] Anchor idle resets within a coalesced HTTP fragment to its receipt time
      and align affected docs.
- [x] Run focused tests and `cargo xtask check`, then audit the diff.
- [x] Run `git add -A`, commit with Conventional Commits, and push the branch.
- [x] Run `cargo xtask review` after the push and report new findings without
      automatically fixing them.
- [x] Choose how to bound and report producer backpressure while preserving
      the idle-time guarantee across separately buffered HTTP fragments.
- [x] Add regressions for buffered fragments, full staging, and fresh output
      received while delivery is blocked; implement bounded reader-side
      decoding and idle timing, and align the docs and public outcome.
- [x] Merge the latest `origin/main`, resolve conflicts, and rerun relevant tests.
- [x] Run focused tests and `cargo xtask check`, then audit the merge and diff.
- [x] Run `git add -A`, commit with Conventional Commits, and push the branch.
- [x] Run `cargo xtask review` after the push and report any findings without
      automatically fixing them.
- [ ] Decide how to address the four post-merge review findings below, then
      add regressions and resolve the chosen follow-ups.
- [ ] Complete the milestone and update the plan index after the review cycle.

### Milestone 7 Review Outcome

The coalesced-fragment fix was committed and pushed in `3904502`.
`cargo xtask check` passed, including 307 tests with three opt-in live tests
ignored. The post-push review identified a remaining case; do not change the
implementation until a maintainer chooses how to handle bounded backpressure.

1. **Severity: high — buffered HTTP fragments can still extend idle time.** At
   `crates/sandbox-e2b/src/process/stream_run.rs:150`, the streaming adapter
   timestamps each HTTP fragment only after the output queue allows it to poll
   that fragment. If the transport has already buffered several fragments, a
   slow consumer draining the 16-slot queue can make old output appear fresh.
   Doing nothing can keep a quiet process running and postpone cleanup until
   the absolute deadline. Option A: read and timestamp provider fragments
   independently of delivery into bounded staging, and terminate with a
   documented outcome when staging fills; this preserves arrival-based idle
   semantics but requires a defined backpressure behavior. Option B: end the
   stream as soon as the delivery queue fills; this is simpler but loses more
   queued output and changes slow-consumer behavior. **Recommendation: A**,
   because it preserves more output while enforcing the advertised idle budget.
   The maintainer chose A and requested a merge from the latest `origin/main`
   after implementation.

### Post-Merge Review Outcome

The independent provider reader and merge from `origin/main` were pushed in
`a12b098` and `fa11e0e`, respectively. `cargo xtask check` passed with all
non-live tests and smoke coverage. The post-push review found four new issues;
do not change the reviewed implementation until the maintainer chooses the
follow-up.

1. **Severity: high — redact command details in stream request diagnostics.**
   `StreamProcessRequest` and `BackendStreamProcessRequest` in
   `crates/sandbox-interface/src/process_stream.rs` and `StreamProcessCommand`
   in `crates/sandbox-e2b/src/process/types.rs` derive `Debug`. Logging one
   currently prints command arguments verbatim, including possible credentials.
   Doing nothing could expose secrets in diagnostic logs. Option A: implement
   metadata-only `Debug` for all three types. Option B: remove `Debug` entirely.
   **Recommendation: A**, matching the existing non-streaming process requests
   while preserving safe structural diagnostics.
2. **Severity: high — redact streamed output in event diagnostics.**
   `ProcessStreamEvent` in `crates/sandbox-interface/src/process_stream.rs`
   derives `Debug` for stdout and stderr byte vectors. Logging those events can
   reveal captured credentials. Doing nothing keeps that leak possible through
   ordinary diagnostics. Option A: implement `Debug` that displays the output
   channel and byte count only. Option B: remove `Debug`. **Recommendation: A**,
   so tests retain safe event visibility without printing output data.
3. **Severity: high — preserve a staged process ID on consumer drop.**
   `crates/sandbox-e2b/src/process/stream_run.rs` checks for consumer closure
   before consuming staged events. If the reader already staged `Started` but
   delivery loses that race, cleanup has no PID and cannot kill the unfinished
   command. Doing nothing can leave a dropped command running until sandbox
   expiry. Option A: publish the observed PID from the reader to shared cleanup
   state. Option B: drain ready start events before honoring closure.
   **Recommendation: A**, removing the scheduling race at its source.
4. **Severity: medium — preserve idle timeout before a process starts.**
   `crates/sandbox-e2b/src/process/stream_run.rs` observes reader timeout
   outcomes only after processing `Started`. A provider stream with no start or
   output instead returns `TransportFailure` when the reader closes. Doing
   nothing misreports an idle timeout to callers. Option A: observe the reader
   outcome before start as well. Option B: add a separate pre-start idle timer.
   **Recommendation: A**, retaining one arrival-based timeout source.
