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
- [ ] After a clean review, mark all milestones complete and move this plan
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
- [ ] Run `git add -A`, commit all completed work with a Conventional Commit,
      push the branch, and confirm GitHub CI passes on that exact commit.
- [ ] Run `cargo xtask review` after the push and record every finding without
      automatically fixing it.
- [ ] After a clean review, mark all milestones complete and move this plan
      from Active to Completed in `plans/README.md`.
