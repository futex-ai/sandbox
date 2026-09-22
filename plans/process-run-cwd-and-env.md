# Add Direct Process Working Directory And Environment

## Summary

Allow trusted direct-argv process consumers to select an optional absolute
working directory and a bounded environment map without changing PTY behavior
or the stateless read-only execution contract. The interface owns the
validation rules, the E2B adapter rejects invalid input before provider access,
and environment values remain secret in diagnostics.

This branch has no streaming process request type. Streaming support remains
out of scope, so this change covers `RunProcessRequest`,
`BackendRunProcessRequest`, and the existing split-stream envd transport only.

## Milestone 1: Define And Validate The Interface Contract

At the end of this milestone, both direct-process request layers expose the
same working-directory and environment fields with one tested validation
contract and redacted diagnostics.

- [x] Add optional `cwd` and ordered `envs` fields to both direct-process
      request types.
- [x] Define interface-owned validation for absolute bounded working
      directories, environment names and values, count and byte limits, and
      template-owned environment names.
- [x] Add typed handled errors that identify invalid environment input without
      including environment values.
- [x] Replace derived request debugging with tests proving environment values
      are redacted.
- [x] Add unit coverage for every accepted boundary and rejection rule.
- [x] Update the sandbox contract and `sandbox-interface` README.

## Milestone 2: Carry The Context Through E2B

At the end of this milestone, valid direct-process context reaches envd's
Connect request exactly, while PTY and read-only behavior remain unchanged.

- [x] Enforce the interface validation before acquiring provider access.
- [x] Thread `cwd` and `envs` through `SplitProcessCommand`, `ProcessCommand`,
      `argv_start`, and the shared process-start encoder.
- [x] Keep PTY locale setup unchanged and make every existing maintenance and
      read-only command pass an explicitly empty environment map.
- [x] Redact environment values from internal command debugging and image
      command failure diagnostics.
- [x] Add Connect wire tests for direct-process `cwd` and `envs`, plus
      regressions for unchanged read-only and PTY behavior.
- [x] Extend the shared conformance probe and alternate backend to verify the
      selected working directory and one environment value.
- [x] Update the E2B adapter documentation and crate README.

## Milestone 3: Validate, Commit, Push, And Review

At the end of this milestone, the complete change is tested, documented,
committed, pushed, and independently reviewed against `origin/main`.

- [x] Run focused validation, wire, adapter, conformance, and redaction tests.
- [x] Run `cargo fmt --all -- --check`, Clippy, the full workspace test suite,
      the file-length lint, smoke coverage, and `cargo xtask check`; fix every
      failure until the complete suite passes.
- [x] Audit tracked files for secrets, generated artifacts, whitespace errors,
      stale documentation, and unrelated edits.
- [x] Run `git add -A`, commit all completed work with a Conventional Commit,
      and push the current branch.
- [x] Run `cargo xtask review` after the push so the AI reviewer checks the
      local diff against `origin/main`; record every finding without
      automatically fixing it.
- [x] Mark every milestone complete and move this plan from Active to
      Completed in `plans/README.md`.

## Milestone 4: Restore Split-Stream Conformance Coverage

At the end of this milestone, the shared cwd and environment probe also
retains its original independent stderr assertion, so a backend cannot pass
conformance while dropping, merging, or mislabeling stderr.

### Review Item

1. **Severity: medium — preserve stderr coverage in the conformance probe.**
   The shared direct-process probe previously emitted and asserted a
   deterministic stderr value. Replacing it with the cwd and environment probe
   made stderr empty, so a backend that mishandles every stderr frame could
   still pass the shared conformance harness. Option A: emit and assert a
   deterministic stderr token in the existing cwd and environment process.
   Option B: add a second stderr-only process probe. **Recommendation: A**,
   because it covers cwd, environment, stdout, and stderr with one provider
   process; the user selected this option.

- [x] Add failing alternate-backend and E2B conformance expectations for the
      restored stderr token.
- [x] Emit the token from the existing shell invocation and assert its exact
      bytes in the shared harness.
- [x] Update the contract, adapter documentation, and crate READMEs to describe
      the combined cwd, environment, stdout, and stderr probe.
- [x] Run focused conformance tests, formatting, Clippy, the full workspace
      test suite, file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for secrets, generated artifacts, whitespace errors,
      stale documentation, and unrelated edits.
- [x] Run `git add -A`, commit the completed follow-up with a Conventional
      Commit, and push the current branch.
- [x] Run `cargo xtask review` after the push so the AI reviewer checks the
      clean local diff against `origin/main`; report every finding without
      automatically fixing it.
- [x] Mark this milestone complete and move the plan from Active to Completed
      in `plans/README.md`.
