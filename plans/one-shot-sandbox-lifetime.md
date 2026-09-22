# One-Shot Sandbox Lifetime

## Summary

Let callers choose between the existing idle auto-pause lifecycle and a bounded
one-shot lifecycle that never pauses or resumes. The shared interface owns the
typed lifetime and validation limit, while the E2B adapter preserves the exact
legacy body for interactive sandboxes and uses a provider destruction timeout
for one-shot work.

## Milestone 1: Define And Validate The Shared Contract

At the end of this milestone, callers and provider backends share one typed,
bounded sandbox-lifetime contract and invalid one-shot requests fail before
provider access.

- [x] Add `SandboxLifetime` with default `IdleAutoPause` and bounded
      `OneShot { max_lifetime }` variants.
- [x] Add `SANDBOX_ONE_SHOT_MAX_LIFETIME` and typed validation for zero,
      over-limit, and provider-unrepresentable durations.
- [x] Carry lifetime through service and backend create requests and expose an
      optional recovered lifetime in managed inventory.
- [x] Update alternate backend, mocks, fixtures, and validation tests for the
      shared interface.

## Milestone 2: Implement E2B One-Shot Lifecycle Semantics

At the end of this milestone, E2B creates one-shot sandboxes without pause or
resume and never extends their original provider timeout while reacquiring
access.

- [x] Encode both create-body variants while preserving the exact existing
      idle auto-pause payload.
- [x] Validate create and recovery requests before any provider access.
- [x] Record and recover the full typed lifetime through provider metadata,
      leaving absent or malformed metadata unknown.
- [x] Route one-shot resume and envd credential reacquisition through the
      non-mutating running-sandbox read path.
- [x] Add focused body, bounds, recovery, metadata, and access-routing tests.

## Milestone 3: Extend Conformance And Live Coverage

At the end of this milestone, every backend proves one-shot create-and-destroy
behavior and the opt-in E2B probe cleans up its bounded resource.

- [x] Add a shared one-shot create-and-destroy conformance probe.
- [x] Update the alternate fake backend and E2B conformance mocks to exercise
      the new probe.
- [x] Add an ignored live E2B one-shot create-and-destroy probe with robust
      cleanup ownership.

## Milestone 4: Align Documentation

At the end of this milestone, consumers and adapter maintainers can implement
and use both lifetime modes without guessing about validation, metadata, or
access behavior.

- [x] Update `docs/sandbox-contract.md` and `docs/e2b-adapter.md`.
- [x] Update the workspace, interface-crate, and E2B-crate READMEs with useful
      lifetime guidance.
- [x] Review the plan and documentation diff for consistency and Markdown
      correctness.

## Milestone 5: Validate, Commit, Push, And Review

At the end of this milestone, the implementation is fully tested, committed,
pushed, and independently reviewed against `origin/main`.

- [x] Run focused tests for the interface, E2B adapter, and conformance suite.
- [x] Run `cargo fmt --all -- --check`, Clippy, the complete workspace tests,
      smoke coverage, file-length lint, and `cargo xtask check`; fix every
      failure until the suite passes.
- [x] Audit the final diff for secrets, generated artifacts, whitespace
      errors, unrelated edits, and documentation drift.
- [ ] Run `git add -A`, commit all completed work with a Conventional Commit,
      and push the current branch.
- [ ] Run `cargo xtask review` after the push and record every finding without
      automatically changing the implementation.
- [ ] Mark this plan complete, move it to the completed index, commit and push
      that bookkeeping, then run a final `cargo xtask review` on the exact
      clean pushed branch.
