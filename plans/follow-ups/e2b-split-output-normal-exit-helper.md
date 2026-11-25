# Split process results lack a normal-exit helper

- **Severity:** Low. **Effort:** Medium. **Confidence:** High. **Harder later:** No.
- **Kind:** Fix after merge.
- **Category:** Code structure.
- **Added:** 2026-11-25, from the branch
  `calummoore/e2b-confirm-resize-cleanup-and-ci-time-7p51`, PR #17, the
  wider fix of review finding 3.
- **Location:** `crates/sandbox-e2b/src/process/types.rs:137`,
  `crates/sandbox-e2b/src/backend/screen_stack.rs:121`,
  `crates/sandbox-e2b/src/backend/screen_resize.rs:144`,
  `crates/sandbox-e2b/src/process/status.rs:6`.
- **Reason:** The fix reaches the process module and screen setup, which the
  branch that found it did not change.

- **Terms:**
  - **Split process result:** the public type `ProcessSplitOutput` at
    `crates/sandbox-e2b/src/process/types.rs:137`. It holds stdout, stderr,
    two overflow flags, `exit_code`, and `exited`.
  - **Normal exit:** a process end that envd reports with `exited: true`. A
    signal end reports `exited: false`, and it can still carry an exit code.
  - **Acknowledgment:** the JSON reply of the resize helper with the version
    and the applied width and height.
- **Background:**
  - Screen setup and resize run the screen helper with `run_split`, and each
    gets a split process result.
  - Screen setup decides success in `succeeded` at
    `crates/sandbox-e2b/src/backend/screen_stack.rs:121`. It requires
    `exited`, `exit_code == Some(0)`, and no overflow.
  - Resize decides the exit in the `match` on
    `(budget, output.exited, output.exit_code)` in the function `resize` of
    `crates/sandbox-e2b/src/backend/screen_resize.rs`. Since 2026-11-25,
    that `match` is the only exit check of resize. `acknowledge` then checks
    only the overflow flags and the JSON.
  - Collected run results already have one shared rule.
    `ProcessRunOutput::succeeded` at
    `crates/sandbox-e2b/src/process/status.rs:6` returns
    `exited && exit_code == Some(0)`. Commit `c96c308` of 2026-09-22 added
    it, and five callers use it.
  - The collector in `crates/sandbox-e2b/src/process/split_run.rs` sets
    `exit_code` and `exited` together from the process end event. Without an
    end event, `exit_code` stays `None`.
  - `crates/sandbox-e2b/src/backend/process_run.rs` copies both fields into
    the interface type `SandboxProcessOutput`. `ProcessSplitOutput` is public,
    because `crates/sandbox-e2b/src/lib.rs` re-exports it.
- **Problem:** Screen setup and resize each read the two exit fields of a
  split process result to decide a normal exit with status zero. No shared
  method owns that rule.
- **Impact if no action:** A change to the normal-exit rule must change each
  copy. A missed copy can accept a signal end that carries status zero. The
  cost is small while only two copies exist.
- **Root question:** How can one place own the normal-exit rule of a split
  process result for every screen command?
- **Options:**
  - **A.** Keep only the fix of 2026-11-25. Resize decides its exit in one
    `match`, and screen setup keeps its own check. Effort: Small.
  - **B.** Give split process results a shared method that returns the
    normal exit status. Effort: Medium.
    - Put the method beside `ProcessRunOutput::succeeded` in
      `crates/sandbox-e2b/src/process/status.rs`. It returns `exit_code`
      only when `exited` is true.
    - Use it in screen setup and in resize. Keep the overflow checks and the
      acknowledgment checks apart from it.
    - Keep every current result.
  - **C.** Store one completion value in `ProcessSplitOutput` in place of
    `exited` and `exit_code`. Effort: Medium.
    - The value has three states: a normal exit with a status, a signal end,
      or no end seen.
    - Screen setup and resize match the value. `process_run.rs` maps it back
      to the two fields of `SandboxProcessOutput`.
    - It changes a public type of the adapter crate in about 15 files,
      mostly test fixtures. It removes two public fields that are on
      `origin/main`, so it needs the user's approval.
  - **D.** Run every resize under the supervisor, with the 15-second budget
    when the request has no deadline. Resize then has one mode and one
    status table. Effort: Medium.
    - Two results of a resize without a deadline change. A hung helper gives
      `BackendUnavailable` after the group kill, not
      `ScreenViewportResizeUnconfirmed`. An outside kill of the started
      process gives `ScreenViewportResizeUnconfirmed`, not
      `BackendUnavailable`.
    - Each such resize also starts `/usr/bin/python3`.
    - The expected command in
      `crates/sandbox-e2b/src/backend/_tests_/screen_stack/resize_tests.rs`,
      the crate README, and `docs/e2b-adapter.md` change.
    - It changes results on `origin/main`, so it needs the user's decision.
      `plans/follow-ups/e2b-resize-without-deadline-confirmation.md` asks
      that question.
- **Left out:**
  - Keep an exit check in `acknowledge` too. It keeps two copies, so it
    fails the goal.
  - One rule for every process result type. Run results already have
    `succeeded`, and other raw reads need other rules, such as status 44 in
    `crates/sandbox-e2b/src/process/file.rs`.
  - A rule in the transport. `run_split` cannot know the status table of
    each caller.
  - A rule in the Python supervisor. The duplicate check is in Rust, and the
    supervisor runs only for a resize with a deadline.
- **Wider fix:** B. Every screen command then uses the shared method, so no
  command writes the rule again. C is the stronger structural fix, because
  the type then cannot hold a state that never occurs, but it needs the
  user.
- **Previous examples:** Commit `c96c308` added `ProcessRunOutput::succeeded`
  at `crates/sandbox-e2b/src/process/status.rs:6` for collected run results.
- **Related:** `plans/follow-ups/e2b-resize-without-deadline-confirmation.md`.
  Its option C is option D of this item. If the user chooses it, apply that
  item first, because resize then has one mode.
- **Recommendation:** B. It gives the rule one home with a small change, and
  it keeps every result. C best protects the code, but it changes a public
  type on `origin/main`.
- **Action:** fix after merge, apply B, because B reaches the process module
  and screen setup, outside the files of the branch that found it.
