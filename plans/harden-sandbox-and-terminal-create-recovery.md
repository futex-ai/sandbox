# Harden Sandbox And Terminal Create Recovery

## Summary

Close two recovery gaps found after completing stream validation and
credential-routing hardening. A sandbox create must either reach the provider
or clearly tell its caller that no request was sent, and terminal recovery must
still identify a terminal whose shell exited before the caller saved its
provider identity.

This is a focused follow-up to the completed extraction and routing-hardening
plans. It preserves the provider-neutral boundary while making its existing
at-most-once creation rules usable during early provider failures and fast
terminal exits.

## Milestone 1: Make Sandbox Dispatch Recoverable

At the end of this milestone, a transient provider inventory failure cannot
move a sandbox create into recovery-only mode before any sandbox was requested.

### Review Item

1. **Severity: high — dispatch sandbox creation before entering recovery-only
   mode.** In `crates/sandbox-e2b/src/backend/sandboxes.rs:62`, a new sandbox
   operation first asks the provider to list matching sandboxes. If that read
   fails temporarily, the adapter returns without sending the create request.
   The caller must treat any started create operation as dispatched and can
   only call the recovery method afterward. Recovery only lists existing
   sandboxes, so it can never find the sandbox that was not created. Doing
   nothing can leave a valid create request stuck until it times out even after
   the provider recovers. Option A: remove the preflight inventory read and
   send the one create request directly. Option B: add an explicit
   pre-dispatch outcome that tells the caller it is safe to attempt creation
   again. **Recommendation: A**, because the caller already guarantees that the
   create request is dispatched at most once, so the extra provider read is not
   needed for duplicate protection.

- [x] Record the finding with its exact location, context, impact, options, and
      recommendation in simple language that assumes no repository context.
- [x] Add a failing regression showing that a new sandbox create does not
      depend on a successful inventory request before dispatch.
- [x] Remove the unnecessary preflight inventory request, or document and test
      a different explicit pre-dispatch outcome if the interface must change.
- [x] Verify ambiguous create delivery still enters recover-only polling and
      cannot send a second provider create request.
- [x] Update the sandbox contract and both crate READMEs with the final
      pre-dispatch and recovery behavior.

## Milestone 2: Recover Terminals That Exit During Creation

At the end of this milestone, terminal recovery can reconstruct the same
provider identity and durable transcript even when the login shell exits before
the original create result is saved.

### Review Item

2. **Severity: medium — preserve exited terminals for creation recovery.** In
   `crates/sandbox-e2b/src/backend/terminals.rs:139`, recovery returns no match
   when the provider's running-process list no longer contains the tagged
   terminal. A terminal can be created successfully and then exit immediately,
   for example because its login profile ends the shell. If the create response
   was interrupted before the caller saved the process ID, the transcript
   remains but the process list no longer contains enough information to
   rebuild the terminal reference. Doing nothing means recovery cannot return
   the already-created terminal, while creating another terminal could replace
   its transcript and duplicate work. Option A: save the process ID and
   terminal identity in trusted provider-side storage, then recover the
   terminal as `Exited` from that record. Option B: keep a tagged supervisor
   process alive as a tombstone until the caller explicitly acknowledges the
   creation result. **Recommendation: A**, because a durable identity record
   survives normal process exit without keeping an otherwise finished process
   alive or adding an acknowledgment protocol.

- [x] Record the finding with its exact location, context, impact, options, and
      recommendation in simple language that assumes no repository context.
- [x] Define the trusted, versioned terminal-identity record and its ownership,
      permissions, lifetime, and compatibility behavior in the protocol docs.
- [x] Add failing regressions for an immediate shell exit after a successful or
      delivery-ambiguous terminal start.
- [x] Persist terminal identity before the terminal can exit, then make
      recovery and inspection return the same provider reference and an
      `Exited` state when only the durable record and transcript remain.
- [x] Ensure input still rejects an exited terminal, close remains idempotent,
      cleanup removes trusted identity state, and PID reuse cannot target an
      unrelated process.
- [x] Update the adapter docs and both crate READMEs with exited-terminal
      recovery and identity-record cleanup behavior.

## Milestone 3: Validate And Review The Recovery Changes

At the end of this milestone, both fixes are fully tested, documented,
committed, pushed, and independently reviewed against `origin/main`.

- [x] Run focused sandbox-create and terminal-recovery regressions.
- [x] Run `cargo fmt --all -- --check`, Clippy, the full workspace test suite,
      the file-length lint, smoke coverage, and `cargo xtask check`; fix every
      failure until the complete suite passes.
- [x] Audit tracked files for prohibited legacy terms, secrets, generated
      artifacts, whitespace errors, and unrelated edits.
- [x] Run `git add -A`, commit all completed work with a Conventional Commit,
      push the branch, and confirm GitHub CI passes on that exact commit.
- [x] Run `cargo xtask review` after the push so the AI reviewer checks the
      clean local diff against `origin/main`; record every finding without
      automatically fixing it.
- [x] After a clean review, mark all milestones complete and move this plan
      from Active to Completed in `plans/README.md`.

### Post-Push Review Findings

3. **Severity: high — initialize terminal storage before identity lookup.** In
   `crates/sandbox-e2b/src/backend/terminal_record.rs:98`, terminal recovery
   tries to read the identity record below `/var/lib/sandbox-e2b/terminals`
   before terminal creation runs the secure directory initializer. On a fresh
   sandbox, or after restored-terminal cleanup removed that directory, the
   production file reader reports an invalid root rather than an absent file.
   Doing nothing means the first terminal create stops before it can dispatch a
   PTY. Option A: run the existing ownership- and symlink-validating directory
   initializer before identity lookup. Option B: distinguish a missing root
   from an unsafe root and treat only the missing-root result as no identity
   record. **Recommendation: A**, because it reuses the existing secure
   initialization path and keeps unsafe roots fail-closed.
4. **Severity: medium — publish identity records only after they are durable.**
   In `crates/sandbox-e2b/src/process/helpers/terminal_transcript.py:206`, the
   wrapper creates the final identity filename before writing and syncing its
   JSON. Recovery can concurrently read an empty or partial record and fail as
   an internal error, or see a complete record before it is durable. Doing
   nothing makes ambiguous-start recovery race terminal initialization and can
   expose identity state that a crash has not safely persisted. Option A:
   write and fsync a private temporary inode, publish it atomically without
   replacing an existing record, then fsync the directory. Option B: retry
   incomplete records while the exact tagged process is still live.
   **Recommendation: A**, because it preserves strict malformed-record handling
   and prevents both partial and pre-durability publication.

## Milestone 4: Close Terminal Identity Review Findings

At the end of this milestone, fresh and restored sandboxes initialize trusted
terminal storage before recovery reads it, and concurrent recovery cannot see
an identity record until its contents have been synced.

- [x] Add failing regressions for fresh-directory identity lookup and
      pre-durability final-name visibility.
- [x] Initialize terminal storage through the existing secure helper before
      create or recovery reads an identity record.
- [x] Write and sync identity data under a private temporary name, publish it
      atomically without replacement, and sync the directory.
- [x] Align the terminal storage documentation with the final publication
      sequence and recovery behavior.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, the file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for prohibited legacy terms, secrets, generated
      artifacts, whitespace errors, and unrelated edits.
- [x] Run `git add -A`, commit all completed work with a Conventional Commit,
      push the branch, and confirm GitHub CI passes on that exact commit.
- [x] Run `cargo xtask review` after the push and record every finding without
      automatically fixing it.
- [x] After a clean review, mark all milestones complete and move this plan
      from Active to Completed in `plans/README.md`.

## Milestone 5: Preserve Transcript Reads After PID Reuse

At the end of this milestone, an exited terminal's durable transcript remains
readable even after an unrelated process reuses its numeric PID, without
weakening terminal identity fencing.

### Review Finding

5. **Severity: medium — allow durable reads after PID reuse.** In
   `crates/sandbox-e2b/src/backend/terminal_output.rs:72`, terminal output calls
   the live-process identity resolver before opening the retained transcript.
   When an exited recorded terminal's numeric PID has been reused by an
   unrelated process, that resolver returns `NotFound`. Recovery and inspection
   already validate the durable identity record and correctly classify the
   terminal as `Exited`, but output returns early and never reads its log. Doing
   nothing makes final shell output inaccessible in long-lived sandboxes even
   though the trusted transcript and matching identity record remain. Option A:
   on this `NotFound` path, validate the durable identity record and continue
   only when it proves the exited terminal. Option B: centralize record-aware
   identity resolution and share it between inspection and output.
   **Recommendation: B**, because one resolution path keeps PID-reuse fencing
   and exited-terminal state consistent across both operations.

- [x] Record the finding with severity, location, context, impact, options, and
      recommendation.
- [x] Add a failing regression for reading a retained exited-terminal
      transcript after unrelated PID reuse.
- [x] Centralize record-aware terminal identity resolution and use it for
      inspection and output reads.
- [x] Align protocol and adapter documentation with the final read behavior.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, file-length lint, smoke coverage, and `cargo xtask check`.
- [ ] Audit the final diff, commit and push the fix, confirm GitHub CI, and run
      a clean post-push `cargo xtask review` against `origin/main`.
- [ ] After a clean review, mark this milestone complete and move the plan back
      to Completed in `plans/README.md`.

## Milestone 6: Validate Live Terminal Identity Records

At the end of this milestone, inspection and output validate every present
durable identity record, including while the exact provider process is live,
while record-free legacy terminals retain their selector-only fallback.

### Review Finding

6. **Severity: medium — validate present records for live terminals.** In
   `crates/sandbox-e2b/src/backend/terminal_record.rs:87`, the shared resolver
   returns `Ready` as soon as the provider lists the exact PID and tag. It does
   not read a durable identity record in that branch, so a live terminal with
   an unknown record version, malformed data, or conflicting identity is
   accepted even though exited terminals fail closed on the same record.
   Doing nothing makes identity validation depend on whether the provider
   process is still running, allowing record corruption or incompatible
   versions to bypass the terminal identity fence until exit. Option A: read
   the optional record first, validate it whenever present, and use the live
   selector only when no record exists. Option B: add a separate optional
   record check only to the live branch. **Recommendation: A**, because one
   record-first path keeps live and exited terminal behavior consistent.

- [x] Record the finding with severity, location, context, impact, options, and
      recommendation.
- [x] Add a failing regression proving a live terminal rejects an incompatible
      identity record.
- [x] Make the shared resolver validate every present identity record and keep
      selector-only fallback for record-free legacy terminals.
- [x] Align protocol and adapter documentation with live-record validation.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit the final diff, run `git add -A`, commit all work with a
      Conventional Commit, push the branch, and confirm GitHub CI passes on
      that exact commit.
- [ ] Run a clean post-push `cargo xtask review` against `origin/main` and
      record any findings without automatically fixing them.
- [ ] After a clean review, mark the remaining milestones complete and move
      this plan back to Completed in `plans/README.md`.

## Milestone 7: Bound And Type Identity Record Reads

At the end of this milestone, terminal identity-file reads finish safely within
the caller's deadline, and only a confirmed missing file enables legacy
selector fallback.

### Review Findings

7. **Severity: medium — bound identity reads inside the output deadline.** In
   `crates/sandbox-e2b/src/backend/terminal_record.rs:118`, every identity-file
   helper receives a fixed ten-second timeout. A terminal output poll has one
   outer deadline equal to the requested wait plus five seconds, so common
   zero-wait reads cancel this helper after roughly five seconds. If the helper
   stream stalls after reporting its process ID, cancellation drops the
   collection future before its normal timeout cleanup can kill that one-shot
   process. Doing nothing lets repeated short polls leave root-authenticated
   helper processes running in the sandbox and breaks the adapter's bounded
   process-cleanup guarantee. Option A: pass the remaining outer deadline into
   the identity read while reserving enough time for confirmed cleanup. Option
   B: make the process collector cancellation-safe so dropping any caller also
   terminates an observed helper. **Recommendation: A**, because it preserves
   the existing single absolute deadline and scopes this fix to the new
   identity-read path.
8. **Severity: medium — treat only file absence as a missing identity.** In
   `crates/sandbox-e2b/src/backend/terminal_record.rs:124`, the wildcard
   `NotFound` match converts every missing-resource error into `None`. The
   regular-file helper reports `ResourceKind::File` when the identity leaf is
   absent, but an envd HTTP 404 while starting that helper is mapped to
   `ResourceKind::Terminal`. Doing nothing can turn provider-level terminal
   disappearance into legacy selector fallback during inspection or hide a
   provider failure as an absent exited terminal during recovery, weakening
   the fail-closed identity contract. Option A: return `None` only for
   `NotFound { resource: ResourceKind::File }` and propagate every other typed
   error. Option B: add a dedicated optional-file-read result at the process
   boundary. **Recommendation: A**, because the existing typed error already
   distinguishes these cases without expanding the interface.

- [x] Record both findings with severity, location, context, impact, options,
      and recommendations.
- [x] Add failing regressions for a stalled identity helper under a zero-wait
      output deadline and for provider-level terminal absence during an
      identity read.
- [x] Thread the caller's remaining output deadline into identity reads while
      preserving bounded cleanup time; keep the standalone inspection and
      recovery read bound explicit.
- [x] Permit legacy fallback only for a confirmed missing identity file and
      propagate provider-level absence unchanged.
- [x] Align protocol, adapter, and crate documentation with the final deadline
      and typed-absence behavior.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit the final diff, run `git add -A`, commit all work with a
      Conventional Commit, push the branch, and confirm GitHub CI passes on
      that exact commit.
- [ ] Run a clean post-push `cargo xtask review` against `origin/main` and
      record any findings without automatically fixing them.
- [ ] After a clean review, mark the remaining milestones complete and move
      this plan back to Completed in `plans/README.md`.

## Milestone 8: Preserve Post-Close Output And Absolute Cleanup Bounds

At the end of this milestone, explicit terminal close retains enough trusted
identity to read final transcript bytes, and output helpers complete or finish
their termination attempt before an earlier absolute helper deadline.

### Review Findings

9. **Severity: high — preserve identity for post-close transcript reads.** In
   `crates/sandbox-e2b/src/backend/terminal_operations.rs:94`, explicit close
   deletes the durable identity record immediately after killing the provider
   process. A later output read then sees neither a live process nor a record
   and returns `NotFound` without opening the retained transcript. Doing
   nothing can lose shell output emitted immediately before close and
   contradicts the adapter contract that keeps the transcript available for
   final ingestion. Option A: retain the trusted identity as an exited
   tombstone until restored-terminal or sandbox cleanup. Option B: make close
   drain and durably persist all final transcript bytes before deleting the
   record. **Recommendation: A**, because it preserves the existing cursor-based
   read API and identity fencing without adding a close-time ingestion path.
10. **Severity: medium — reserve cleanup from an absolute helper deadline.** In
    `crates/sandbox-e2b/src/backend/terminal_output.rs:184`, output subtracts
    the three-second kill timeout from a relative duration before dispatching
    the identity helper. The production collector starts that relative timer
    only after request setup, so its execution and full kill attempt can still
    extend past the outer deadline and be cancelled. Doing nothing means a
    stalled root-authenticated helper can survive repeated short polls despite
    the documented cleanup reserve. Option A: carry an earlier absolute helper
    completion deadline through the regular-file request and collector. Option
    B: reserve extra timing margin and decline dispatch when the remaining
    duration is too short. **Recommendation: A**, because an absolute deadline
    makes the guarantee independent of request-setup time.

- [x] Record both findings with severity, location, context, impact, options,
      and recommendations.
- [x] Add failing regressions for final transcript reads after repeated close
      and for absolute helper-deadline propagation into collection and cleanup.
- [x] Retain the durable terminal identity through explicit close and remove it
      only with restored-terminal or sandbox cleanup.
- [x] Carry an absolute identity-helper completion deadline through the process
      transport and reserve execution, termination, and return budgets before
      dispatch.
- [x] Align protocol, adapter, and crate documentation with post-close reads
      and absolute helper cleanup bounds.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, file-length lint, smoke coverage, and `cargo xtask check`.
- [ ] Audit the final diff, run `git add -A`, commit all work with a
      Conventional Commit, push the branch, and confirm GitHub CI passes on
      that exact commit.
- [ ] Run a clean post-push `cargo xtask review` against `origin/main` and
      record any findings without automatically fixing them.
- [ ] After a clean review, mark the remaining milestones complete and move
      this plan back to Completed in `plans/README.md`.
