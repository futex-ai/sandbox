# File-read requests print caller paths in Debug

- **Severity:** Medium. **Effort:** Small. **Confidence:** High. **Harder later:** No.
- **Kind:** Needs decision.
- **Category:** Security.
- **Added:** 2026-11-04, from the branch `conductor/agents-md-sandbox`. The
  analysis of the deleted streaming plans found it. No plan named it.
- **Location:** `crates/sandbox-interface/src/requests.rs:264-279`,
  `crates/sandbox-interface/src/backend_files.rs:5-18`,
  `crates/sandbox-interface/src/read_only.rs:55-68`,
  `crates/sandbox-e2b/src/process/types.rs:189-204`.
- **Reason:** The fix adds a requirement for the diagnostics of file-read
  requests, and the user has not reviewed one yet.

- **Terms:**
  - **File-read request:** a public type that asks to read a sandbox file
    below a root: `ReadFileRequest`, `BackendReadFileRequest`,
    `ReadOnlyFileRequest`, or E2B's `ProcessRegularFileRequest`.
  - **Metadata-only `Debug`:** a hand-written `Debug` implementation that
    prints identifiers, counts, limits, and flags, but no caller text and no
    payload bytes.
- **Background:**
  - The four file-read requests derive `Debug` with their `root` and `path`
    fields.
    - `ReadFileRequest` at `crates/sandbox-interface/src/requests.rs:264-279`,
      with the derive at line 265. Its fields are `owner`, `sandbox_id`,
      `root`, `path`, `offset`, and `length`.
    - `BackendReadFileRequest` at
      `crates/sandbox-interface/src/backend_files.rs:5-18`, with the derive at
      line 6. Its fields are `sandbox_provider_ref`, `root`, `path`, `offset`,
      and `max_bytes`.
    - `ReadOnlyFileRequest` at `crates/sandbox-interface/src/read_only.rs:55-68`,
      with the derive at line 56. Its fields are `owner`, `sandbox_id`, `root`,
      `path`, and `output_limit`.
    - E2B's `ProcessRegularFileRequest` at
      `crates/sandbox-e2b/src/process/types.rs:189-204`, with the derive at
      line 190. Its fields are `root`, `path`, `offset`, `max_bytes`,
      `timeout`, and `completion_deadline`.
  - Trusted service code selects the `root`. The `path` is relative to the
    root, and it can come from the caller.
  - `ProviderRef` already hides its contents in `Debug`, as
    `docs/process-diagnostics.md:48-50` says. So `BackendReadFileRequest`
    prints no provider reference.
  - The matching write requests hide these fields. On 2026-10-03 the user
    approved a metadata-only `Debug` for `WriteFileRequest`,
    `BackendWriteFileRequest`, and E2B's `ProcessRegularFileWriteRequest`.
  - That record says that the root, the path, the bytes, and the provider
    reference are omitted. Its reason is that this matches "the
    process-diagnostics rule that caller paths are omitted". The code is at
    `crates/sandbox-interface/src/diagnostics/file.rs:7-26` and
    `crates/sandbox-e2b/src/process/command_debug.rs:10-17`.
  - No requirement covers file-read requests.
    `docs/process-diagnostics.md:21-27` lists the `Debug` implementations
    that omit paths: process requests, image preparation requests,
    file-write requests, and captured output. `docs/sandbox-contract.md:371-374`
    names only "Process, terminal, and file-write request `Debug`".
  - Today no code in this workspace formats these requests. Tracing uses
    static text and selected metadata, as `docs/process-diagnostics.md:57-58`
    requires.
- **Problem:** The four file-read requests print the root and the path of
  the caller in `Debug`. The write requests have hidden the same fields since
  2026-10-03. No requirement says what the diagnostics of a read request may
  show.
- **Impact if no action:** A consumer that logs a file-read request writes
  the file path of the caller to its logs. A path can hold a user name, a
  customer name, or another private value. The rules for reads and writes
  stay different, and no record gives a reason.
- **Root question:** What may the automatic diagnostics of a file-read
  request show?
- **Options:**
  - **A.** Keep the derived `Debug`, and state it as a requirement. Add one
    sentence to `docs/process-diagnostics.md`. It says that file-read request
    diagnostics may show the root and the path, because a path is not file
    content. Reads and writes then follow different rules. Effort: Small.
  - **B.** Remove `Debug` from the four types. It removes a public trait
    implementation, so code that formats these values stops compiling. On
    2026-09-23 the maintainer rejected this alternative for the streaming
    request and event types. Effort: Small.
  - **C.** Give the four types a metadata-only `Debug` that omits `root`,
    `path`, and the provider reference. Effort: Small.
    - Print `ReadFileRequest { owner, sandbox_id, offset, length }`,
      `BackendReadFileRequest { offset, max_bytes }`,
      `ReadOnlyFileRequest { owner, sandbox_id, output_limit }`, and
      `ProcessRegularFileRequest { offset, max_bytes, timeout, completion_deadline }`.
    - Put the interface code in
      `crates/sandbox-interface/src/diagnostics/file.rs`. Put the E2B code in
      `crates/sandbox-e2b/src/process/command_debug.rs`, beside the write
      request.
    - Test `{:?}` and `{:#?}` with `assert_metadata_only` from
      `crates/sandbox-interface/tests/request_diagnostics.rs:207-224`. That
      helper already rejects the fields ` root:` and ` path:`. The test file
      has 232 lines, and `xtask/src/file_length.rs:10` caps a Rust file at 300
      lines. So put the new tests in a new test file.
    - Add the file-read requests to the list in
      `docs/process-diagnostics.md:21-27`. The summary at
      `docs/sandbox-contract.md:370-386` states no rule for file-read
      requests, so it can stay as it is.
    - If the change edits `docs/sandbox-contract.md`, the change must also
      split it. That doc has 428 lines. The `documentation` skill from
      `futex-ai/skills` says: "When you touch a doc that passes the target,
      split it into linked pages in the same change."
  - **D.** Add a shared caller-path type whose `Debug` prints only its byte
    length. Use it for every root and path field of every request. It
    changes public field types on `origin/main`, so it needs the approval of
    the user. Effort: Large.
- **Left out:**
  - Mask the secret-looking parts of a path. `docs/sandbox-contract.md:373-375`
    says that diagnostics omit caller text "instead of matching known
    secrets".
  - Show the trusted root and hide only the path. It differs from C in one
    field only, and the write rule of 2026-10-03 hides the root too.
  - Add a test that checks the `Debug` of every public type. It only detects
    a leak. It does not decide what the diagnostics show.
- **Wider fix:** D. It is blocked, because it changes public types on
  `origin/main`.
- **Previous examples:** The file-write rule of 2026-10-03 at
  `crates/sandbox-interface/src/diagnostics/file.rs:7-26`. The process
  requests show `has_cwd` instead of the working directory, at
  `crates/sandbox-interface/src/diagnostics/process.rs:11-25`.
  `ProcessPtyRequest` does the same at
  `crates/sandbox-e2b/src/process/command_debug.rs:63-70`.
- **Related:**
  - `plans/follow-ups/file-read-results-debug-prints-bytes.md`: the same
    class, caller data in a derived `Debug`. Both change
    `docs/process-diagnostics.md`. After the user answers this item, one
    branch can apply both.
  - `plans/follow-ups/control-http-debug-prints-token-bytes.md`: the same
    class, in two crate-private E2B types. That item does not wait for the
    answer to this item.
- **Recommendation:** C, because it applies the rule of 2026-10-03, that
  diagnostics omit caller paths, to the matching read requests. D protects
  the codebase best, but it changes public types.
- **Action:** ask the user, because no requirement covers the diagnostics of
  file-read requests, so option C adds a requirement at the boundary.

Record of the write-request rule, at the commit before the plan deletion:
<https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/stream-process-cwd-env-and-write-redaction.md?plain=1#L62-L72>
