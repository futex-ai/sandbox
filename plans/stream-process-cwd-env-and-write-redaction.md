# Stream Process Working Directory, Environment, And Write Redaction

## Summary

Let trusted callers choose a working directory and environment variables for
streaming process runs, the same way they already can for collected
`run_process` runs. Today `StreamProcessRequest` and
`BackendStreamProcessRequest` have no `cwd` or `envs` fields. The E2B
streaming transport also encodes envd's `Start` request with no working
directory and an empty environment map: `stream_events` in
`crates/sandbox-e2b/src/process/stream_run.rs` calls `argv_start` with `None`
and `BTreeMap::new()`. The `Start` request already has `process.cwd` and
`process.envs` fields, and the collected path already fills them, so this
change threads two values through existing layers.

The same change fixes one related leak. `WriteFileRequest`,
`BackendWriteFileRequest`, and the E2B transport's
`ProcessRegularFileWriteRequest` derive `Debug`, so formatting a write request
prints the file contents. The consumer build writes a GitHub token through this
path, and E2B image realization writes its input files through
`ProcessRegularFileWriteRequest`. All three types get metadata-only `Debug`.

Struct-literal breaking changes are acceptable because the workspace is not yet
in production.

## Design Decisions

1. **Reuse the collected-run validation unchanged.** Make the private
   `validate_execution_context` helper in
   `crates/sandbox-interface/src/process_run.rs` `pub(crate)` and add a
   `validate_execution_context()` method to both stream request types in
   `process_stream.rs`. Rules, limits, and errors stay identical: an absolute
   cwd of at most 4,096 bytes with no control characters; names matching
   `[A-Za-z_][A-Za-z0-9_]*`; no NUL in values; at most 256 entries and 64 KiB;
   `PATH`, `HOME`, `LD_*`, and `DYLD_*` rejected. Failures return
   `Error::InvalidProcessRunContext { reason }` without echoing input.
   `ProcessRunContextError` keeps its name; only its doc comment changes.
2. **Use the `run_process` field shape and position.** Add
   `cwd: Option<String>` and `envs: BTreeMap<String, String>` after `args` on
   `StreamProcessRequest`, `BackendStreamProcessRequest`, and E2B's
   `StreamProcessCommand`, reusing the run-request doc comments. Absent values
   (`None` and an empty map) produce today's exact `Start` body.
3. **Validate before provider access, in the collected-run order.** E2B's
   stream `validate` checks the execution context first, then argv, output
   limits, deadline, and idle timeout, all before the control API is asked for
   sandbox access.
4. **Keep transport-layer parity.** `ConnectProcessTransport::stream_process`
   keeps validating only its timers, like `run_split`. Context validation
   stays at the backend boundary, where `run_process` performs it.
5. **Diagnostics expose flags and counts only.** Stream `Debug` adds
   `has_cwd` and `env_count` after `arg_count`, matching `RunProcessRequest`:
   - `StreamProcessRequest { owner, sandbox_id, arg_count, has_cwd, env_count,
     stdout_limit, stderr_limit, deadline, idle_timeout }`
   - `BackendStreamProcessRequest { arg_count, has_cwd, env_count,
     stdout_limit, stderr_limit, deadline, idle_timeout }`
   - `StreamProcessCommand { arg_count, has_cwd, env_count, stdout_limit,
     stderr_limit, requested_at, deadline, idle_timeout }`

   E2B's private `StreamSettings` holds the encoded `Start` body, which now
   contains environment values, so it must stay without `Debug`. No tracing
   event may record a cwd, an environment name or value, or an encoded body.
6. **Write-request diagnostics show identifiers and a byte count.**
   - `WriteFileRequest { lifecycle_operation_id, owner, sandbox_id,
     input_bytes }`
   - `BackendWriteFileRequest { input_bytes }`
   - `ProcessRegularFileWriteRequest { input_bytes }`

   `root`, `path`, `bytes`, and the provider reference are omitted. This
   matches `RealizeImageFileInput` and `BackendInputRequest`, which already
   carry build input and terminal input, and the process-diagnostics rule that
   caller paths are omitted. Field access, equality, and the bytes written to
   the sandbox do not change.
7. **Extend the existing streaming conformance probe.** As with the collected
   probe (Milestone 4 of
   [process-run-cwd-and-env.md](process-run-cwd-and-env.md)), one process
   covers cwd, environment, stdout, and stderr. The probe sends cwd
   `/workspace`, `SANDBOX_PROBE=stream-environment-map`, and the script
   `pwd; printf '%s' "$SANDBOX_PROBE"; printf '%s' 'stream-stderr' >&2`. It
   expects stdout `/workspace\nstream-environment-map` and stderr
   `stream-stderr`. The value differs from the collected probe's
   `environment-map`, so a backend cannot pass by reusing the earlier run's
   context. Conforming images already provide `/bin/sh` and `/workspace`.

## Out Of Scope

- File-read results `FileContent` and `BackendFileContent` also derive `Debug`
  with raw bytes, so reading the token file back would print it. E2B's
  `ProcessFileChunk` is already redacted. Fix this in a separate follow-up
  plan.
- Live E2B verification. The opt-in live suite runs neither streaming nor
  `exercise_backend`. Streaming uses the same envd `Start` RPC and
  `ProcessConfigWire` that collected runs already use to send `cwd` and `envs`.
- Renaming `ProcessRunContextError`, adding context validation for direct
  `ProcessTransport` callers, and any PTY or stateless read-only change.

## Milestone 1: Forward Streaming Working Directory And Environment (Completed)

At the end of this milestone, both stream request layers carry a validated
`cwd` and `envs`, and E2B sends them in the streaming `Start` request.
Diagnostics show only `has_cwd` and `env_count`, and the shared conformance
harness proves that a backend honors both values. Interface and adapter
changes land together, so no backend silently drops a valid value.

Interface (`crates/sandbox-interface`):

- [x] Add `cwd` and `envs` to `StreamProcessRequest` and
      `BackendStreamProcessRequest`. Document that backends must call
      `validate_execution_context` before provider access.
- [x] Make `process_run::validate_execution_context` `pub(crate)`, add the two
      stream `validate_execution_context()` methods, and extend the
      `ProcessRunContextError` doc comment to cover streaming runs.
- [x] Add `has_cwd` and `env_count` to both stream `Debug` impls in
      `src/diagnostics/process.rs`.
- [x] Add `src/_tests_/process_stream_tests.rs`, declared from
      `process_stream.rs`. Use the collected-run case matrix: exact accepted
      bounds and absent context; relative, oversized, NUL, and
      control-character cwd; invalid, `PATH`, `HOME`, `LD_*`, and `DYLD_*`
      names; a NUL value; 257 entries; and one byte over 64 KiB. Assert that
      both stream layers return the same result as `BackendRunProcessRequest`,
      and assert the exact reason for representative rejections.
- [x] Extend the stream case in `tests/process_diagnostics.rs` with a secret
      cwd, environment name, and environment value. Assert the exact backend
      `Debug` string and that ordinary and pretty `Debug` for both layers omit
      the secret, `cwd:`, and `envs:`. Assert that explicit fields stay exact
      and that stream validation errors do not echo a name or value.

E2B adapter (`crates/sandbox-e2b`):

- [x] Add `cwd` and `envs` to `StreamProcessCommand`. `src/process/types.rs`
      grows from 289 to about 293 lines and must stay under 300.
- [x] Add `src/process/_tests_/stream_run/context_tests.rs` before wiring the
      values (destructure the new fields as `_` until then). Capture the body
      passed to `stream_with_timeout` for `Start` and assert that
      `process.cwd` and `process.envs` match the command. With absent context,
      assert that `cwd` is omitted and `envs` is `{}`, which is today's body.
      Confirm the first case fails before the fix.
- [x] Pass `cwd` and `envs` to `argv_start` in `stream_events`, and remove the
      unused `BTreeMap` import.
- [x] In `src/backend/process_stream.rs`, call
      `request.validate_execution_context()` first in `validate`, and forward
      both fields into `StreamProcessCommand`.
- [x] Add `src/backend/_tests_/process_stream_context_tests.rs`, declared in
      `src/backend/mod.rs`, mirroring the collected-run tests. An invalid cwd
      and a template-owned name must fail with typed reasons while the control
      and process mocks expect no calls. A valid context must reach the
      transport command exactly.
- [x] Add `has_cwd` and `env_count` to `StreamProcessCommand` `Debug`, and
      extend `stream_command_debug_omits_argv_and_reports_limits` with a secret
      cwd and environment entry.
- [x] Add a doc comment to `StreamSettings` stating that its encoded `Start`
      body contains environment values and must never implement `Debug`.
- [x] Add `cwd: None` and an empty map to the remaining struct literals in
      `backend/_tests_/process_stream_tests.rs`, `stream_run/backend_tests.rs`,
      `stream_run/support.rs`, `connect_duration_tests.rs`, and
      `command_debug_tests.rs`.

Conformance:

- [x] Update `crates/sandbox-interface/src/conformance_process_stream.rs` to
      send the Decision 7 cwd, environment, and script, and to require their
      exact stdout and stderr.
- [x] Make the alternate backend
      (`crates/sandbox-interface/tests/backend_conformance/alternate_process.rs`)
      and the E2B fixture
      (`crates/sandbox-e2b/src/backend/_tests_/backend_conformance_support.rs`)
      assert the exact cwd and environment they receive and return the
      matching output.
- [x] Add `src/_tests_/conformance_process_stream_tests.rs`, declared from
      `conformance_process_stream.rs`, using
      `SandboxBackendMock::stream_process`. A backend that returns the expected
      output passes; a backend that ignores the context and returns stdout
      `/home/user\n` fails the probe.

Docs:

- [x] Update `docs/sandbox-contract.md` for the streaming request fields,
      shared validation before provider access, and the streaming probe.
      Update `docs/process-diagnostics.md` so streaming diagnostics list
      `has_cwd` and `env_count`.
- [x] Update `docs/e2b-adapter.md` so the streaming `Start` request carries
      `cwd` and `envs`, and describe the `StreamProcessCommand` diagnostics and
      the streaming probe.
- [x] Add to `docs/juno-adoption.md` that the service must validate the
      `StreamProcessRequest` context before provider access and copy `cwd` and
      `envs` into `BackendStreamProcessRequest`. This workspace has no service
      implementation.
- [x] Update the root `README.md`, `crates/sandbox-interface/README.md`, and
      `crates/sandbox-e2b/README.md` passages on direct-process context,
      streaming diagnostics, and the streaming probe.
- [x] Run `cargo test -p sandbox-interface` and `cargo test -p sandbox-e2b`,
      and fix failures until both pass.

## Milestone 2: Redact File-Write Request Diagnostics (Completed)

At the end of this milestone, formatting any file-write request with `{:?}` or
`{:#?}` shows identifiers and a byte count, never the file path or contents.
The bytes written to the sandbox stay exact.

- [x] Add failing tests first:
  - [x] In `crates/sandbox-interface/tests/request_diagnostics.rs`, build
        `WriteFileRequest` and `BackendWriteFileRequest` with the payload as
        root, path, and bytes. Run both through `assert_metadata_only`, assert
        the exact `BackendWriteFileRequest { input_bytes: N }` string, and
        assert that explicit `root`, `path`, and `bytes` are unchanged.
  - [x] In `crates/sandbox-e2b/src/process/_tests_/command_debug_tests.rs`,
        assert that `ProcessRegularFileWriteRequest` formats as
        `ProcessRegularFileWriteRequest { input_bytes: N }` in both forms and
        omits a token-shaped payload.
  - [x] Confirm the new tests fail against the derived `Debug` of all three
        types.
- [x] Remove `Debug` from the derives of `WriteFileRequest`
      (`src/requests.rs`, a derive-only edit to a 296-line file) and
      `BackendWriteFileRequest` (`src/backend_files.rs`). Add
      `src/diagnostics/file.rs` with the Decision 6 impls and declare it in
      `src/diagnostics/mod.rs`.
- [x] Remove `Debug` from the `ProcessRegularFileWriteRequest` derive and
      implement it in `src/process/command_debug.rs`. Update that module's doc
      comment to cover file-write requests.
- [x] Confirm that the existing write-path tests (`file_transfer_tests.rs`,
      the `regular_file_write*_tests.rs` files, and conformance file transfer)
      still pass, proving written bytes stay exact.
- [x] Update `docs/process-diagnostics.md` and `docs/sandbox-contract.md` to
      list file-write requests among the metadata-only diagnostics. Update the
      two crate READMEs and the root `README.md` summary to match.
- [x] Run `cargo test -p sandbox-interface` and `cargo test -p sandbox-e2b`,
      and fix failures until both pass.

## Milestone 3: Validate, Commit, Push, And Review (Completed)

At the end of this milestone, the complete change is checked, committed,
pushed, and reviewed against `origin/main`.

- [x] Run `cargo fmt --all -- --check` (on failure, run `cargo fmt --all` and
      check again), Clippy with warnings denied, the full workspace tests,
      `cargo xtask rust-file-length-lint --all`, `cargo xtask smoke-test`, and
      `cargo xtask check`. Fix every failure until all of them pass.
- [x] Search diagnostics and tracing for any remaining code that formats
      `cwd`, `envs`, an encoded `Start` body, or write bytes.
- [x] Audit tracked files for secrets, generated artifacts, whitespace errors
      (`git diff --check`), stale docs, and unrelated edits.
- [x] Run `git add -A`, commit with the Conventional Commit entries
      `feat(stream): forward process cwd and env` and
      `fix(files): redact write contents from Debug`, and push the current
      branch.
- [x] Repair the reviewer environment by installing `bubblewrap` and dropping
      inherited VM capabilities for the review subprocess; retry the review.
- [x] Run `cargo xtask review` after the push. Report every finding as a
      numbered item with a severity, plain-language context, the impact of
      doing nothing, lettered options, and a recommendation. Do not fix
      findings automatically.
- [x] Mark every milestone complete and move this plan from Active to
      Completed in `plans/README.md`.
