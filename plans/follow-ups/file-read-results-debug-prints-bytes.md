# File-read results print raw bytes in Debug

- **Severity:** High. **Effort:** Medium. **Confidence:** High. **Harder later:** No.
- **Kind:** Fix after merge.
- **Category:** Security.
- **Added:** 2026-11-04, from the branch `conductor/agents-md-sandbox`. It
  holds the deferred work of the deleted plan
  `plans/stream-process-cwd-env-and-write-redaction.md`, lines 86 to 89,
  which the user approved on 2026-10-03.
- **Location:** `crates/sandbox-interface/src/domain.rs:192`,
  `crates/sandbox-interface/src/backend_files.rs:34`,
  `crates/sandbox-interface/src/read_only.rs:71`,
  `docs/process-diagnostics.md:42-43`, `docs/sandbox-contract.md:385-386`.
- **Reason:** The approved plan of 2026-10-03 sent this fix to a separate
  change, and the branch that deleted that plan changed no code.

- **Terms:**
  - **File-read result:** a public type that carries bytes read from a
    sandbox file: `FileContent`, `BackendFileContent`, or
    `ReadOnlyFileOutput`.
  - **Metadata-only `Debug`:** a hand-written `Debug` implementation that
    prints counts, limits, and flags, but no payload bytes and no caller
    text.
- **Background:**
  - `sandbox-interface` has three file-read results. Each one derives
    `Debug` with its `bytes: Vec<u8>` field.
    - `FileContent` at `crates/sandbox-interface/src/domain.rs:191-202`. The
      derive is at line 192, and `bytes` is at line 197. It also has
      `offset`, `total_size`, and `truncated`.
    - `BackendFileContent` at
      `crates/sandbox-interface/src/backend_files.rs:33-40`. The derive is at
      line 34, and `bytes` is at line 37. It also has `total_size`.
    - `ReadOnlyFileOutput` at `crates/sandbox-interface/src/read_only.rs:70-79`.
      The derive is at line 71, and `bytes` is at line 74. It also has
      `total_size` and `truncated`. It is the result of
      `SandboxReadOnly::read_only_file`.
  - A derived `Debug` prints a `Vec<u8>` as a list of decimal byte values. A
    reader can decode that list back to the file contents.
  - The approved plan of 2026-10-03 records that a consumer build writes a
    GitHub token into a sandbox file through `write_file`. When the consumer
    reads that file back, the token is in `bytes`, and `{:?}` prints it.
  - The E2B adapter builds `BackendFileContent` from `ProcessFileChunk` at
    `crates/sandbox-e2b/src/backend/files.rs:44-47`. `ProcessFileChunk`
    already has a metadata-only `Debug` with `output_bytes` and
    `total_size`, at `crates/sandbox-e2b/src/process/output_debug.rs:45-53`.
  - Today no code in this workspace formats these values. Every `tracing`
    event uses static text and selected metadata. The consumer implements
    `SandboxService` and `SandboxReadOnly`, so consumer code can format the
    results.
  - The current rule is in `docs/process-diagnostics.md:42-43`: "File-read
    results are outside this write-request guarantee and must still be
    treated as sensitive data." `docs/sandbox-contract.md:385-386`,
    `crates/sandbox-interface/README.md:84`, and
    `crates/sandbox-e2b/README.md:228` say the same.
  - `docs/process-diagnostics.md:3-5` says that the library does not try to
    discover or replace secrets in process and terminal data.
  - The user decision. On 2026-10-03 the user approved the plan that gave
    the file-write requests a metadata-only `Debug`. That plan named this
    leak and sent its fix to a separate change. It named `FileContent` and
    `BackendFileContent`, but it missed `ReadOnlyFileOutput`. This approval
    covers the change of the rule at `docs/process-diagnostics.md:42-43`.
  - On 2026-09-23 the maintainer rejected the removal of `Debug` from the
    streaming request and event types. The reason was that tests and
    callers need safe structural diagnostics.
- **Problem:** The three file-read results print the bytes of the file in
  `Debug`. A file can hold a credential, such as the GitHub token above. The
  docs only tell consumers to treat these results as sensitive.
- **Impact if no action:** A consumer that logs a file-read result writes the
  file contents, and any credential in them, to its logs. Every other public
  output type of the library already hides its bytes, so a consumer can
  expect the same here.
- **Root question:** What may the automatic diagnostics of a file-read result
  show?
- **Options:**
  - **A.** Replace the derived `Debug` of the three types with a
    metadata-only `Debug`. Effort: Medium.
    - Print `FileContent { offset, output_bytes, total_size, truncated }`,
      `BackendFileContent { output_bytes, total_size }`, and
      `ReadOnlyFileOutput { output_bytes, total_size, truncated }`. The name
      `output_bytes` matches `ProcessFileChunk` and `BackendTerminalOutput`.
    - Put the code in `crates/sandbox-interface/src/diagnostics/file.rs`,
      which has 26 lines.
    - Test `{:?}` and `{:#?}` with `assert_metadata_only` and exact strings,
      as `crates/sandbox-interface/tests/request_diagnostics.rs:17-65` and
      `:207-224` do. Assert that `bytes` stays exact.
    - That test file has 232 lines, and `xtask/src/file_length.rs:10` caps a
      Rust file at 300 lines. So put the new tests in a new test file.
    - Update `docs/process-diagnostics.md:21-27` and `:42-43`,
      `docs/sandbox-contract.md:370-386`,
      `crates/sandbox-interface/README.md:81-84`,
      `crates/sandbox-e2b/README.md:224-228`, and `README.md:106-108`.
    - `docs/sandbox-contract.md` has 428 lines. The `documentation` skill
      from `futex-ai/skills` says: "When you touch a doc that passes the
      target, split it into linked pages in the same change." The target is
      about 250 lines. So this change splits that doc, unless an earlier
      change did.
  - **B.** Remove `Debug` from the three types. It removes a public trait
    implementation, so code that formats these values stops compiling. It
    repeats the alternative that the maintainer rejected for the streaming
    types on 2026-09-23. Effort: Small.
  - **C.** Change nothing, and keep the rule that consumers treat file-read
    results as sensitive. The leak path stays. Effort: Small.
  - **D.** Add a shared payload-bytes type whose `Debug` prints only a
    length. Use it for every payload field of the library. It changes the
    public field types of many types on `origin/main`, so it needs the
    approval of the user. Effort: Large.
- **Left out:**
  - Mask secrets in the printed bytes. `docs/process-diagnostics.md:3-5`
    says that the library does not discover or replace secrets.
  - Add a test that checks the `Debug` of every public type. It only detects
    a leak. It does not decide what the diagnostics show.
- **Wider fix:** D. It is blocked, because it changes public types on
  `origin/main`. The branch that applies A records D as its own follow-up
  item of the kind Needs decision.
- **Previous examples:**
  - Streaming requests and events, decided on 2026-09-23:
    `crates/sandbox-interface/src/diagnostics/process.rs:27-79`.
  - File-write requests, decided on 2026-10-03:
    `crates/sandbox-interface/src/diagnostics/file.rs:7-26` and
    `crates/sandbox-e2b/src/process/command_debug.rs:10-17`.
  - Output types: `ReadOnlyExecOutput` at
    `crates/sandbox-interface/src/diagnostics/process.rs:133-141`,
    `BackendTerminalOutput` at
    `crates/sandbox-interface/src/diagnostics/terminal.rs:28-40`, and the E2B
    outputs at `crates/sandbox-e2b/src/process/output_debug.rs:8-53`.
- **Related:**
  - `plans/follow-ups/file-read-requests-debug-prints-paths.md`: the same
    class, caller data in a derived `Debug`. Both change
    `docs/process-diagnostics.md`. This item does not wait for the answer to
    that item.
  - `plans/follow-ups/docs-miss-decisions-in-force.md`: both change
    `docs/sandbox-contract.md`. The first change that lands splits it.
  - `plans/follow-ups/control-http-debug-prints-token-bytes.md`: the same
    class, raw bytes in a derived `Debug`, in two crate-private E2B types.
    Neither item waits for the other.
- **Recommendation:** A, because it matches the metadata-only `Debug` of every
  other public request, event, and output type. On 2026-10-03 the user
  approved a fix of this leak in a separate change. D protects the codebase
  best, because no new payload field can leak, but it needs the user.
- **Action:** fix after merge, apply A, because the branch that recorded this
  item changed only docs, plans, and agent rules.

Source record, at the commit before the deletion:
<https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/stream-process-cwd-env-and-write-redaction.md?plain=1#L86-L89>
