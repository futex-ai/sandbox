# Resize supervisor faults lack a test each

- **Severity:** Low. **Effort:** Medium. **Confidence:** Medium. **Harder later:** No.
- **Kind:** Needs decision.
- **Category:** Test.
- **Added:** 2026-11-25, from the branch
  `calummoore/e2b-confirm-resize-cleanup-and-ci-time-7p51`, PR #17, the
  wider fix of review finding 2.
- **Location:** `crates/sandbox-e2b/tests/screen_resize_deadline.rs:198`,
  `crates/sandbox-e2b/src/backend/screen_resize.rs:50`.
- **Reason:** A fault table is a test for a class of defects, and the
  always-on rules keep that choice for the user.

- **Terms:**
  - **Resize helper:** the template program at the configured screen-helper
    path that changes the sandbox viewport.
  - **Supervisor:** the trusted Python script `RESIZE_SUPERVISOR` in
    `crates/sandbox-e2b/src/backend/screen_resize.rs`. It runs the resize
    helper when the resize request has a deadline.
  - **Proven status:** a normal supervisor exit with status 0, 3, or 124.
    Only these statuses prove that the helper ended.
  - **Fault prologue:** Python code that the Linux test puts before the
    supervisor script. It replaces one call with a function that raises an
    error.
- **Background:**
  - On 2026-11-24 the user chose the rule for a resize with a deadline. Only
    three normal supervisor exits prove that the helper ended. Status 0 means
    that the helper succeeded. Status 3 means that the helper failed and the
    supervisor reaped it. Status 124 means that the supervisor killed the
    helper's process group and reaped the helper.
  - The adapter returns `BackendUnavailable` for a proven failure. Every
    other outcome returns `ScreenViewportResizeUnconfirmed`, so the caller
    keeps its session fence. The section "Files, Processes, Terminals, And
    Screens" of `docs/e2b-adapter.md` states this rule.
  - An error that the script does not catch makes Python exit with status 1.
    The adapter then returns `ScreenViewportResizeUnconfirmed`, which is
    correct.
  - After its setup, the script makes five calls that can fail.
    `subprocess.Popen` starts the helper. `signal.sigtimedwait` waits for a
    signal. `os.waitid` looks for a helper exit. `Popen.wait` reaps the
    helper. `os.killpg` kills the helper's group, and the script catches only
    its `ProcessLookupError`.
  - The Linux test `crates/sandbox-e2b/tests/screen_resize_deadline.rs`
    runs the real script with `/usr/bin/python3`. Its clock prologue already
    replaces `time.monotonic`.
  - Since 2026-11-25 the test has two negative cases. A helper path that
    does not exist makes `subprocess.Popen` fail. A fault prologue makes
    `os.waitid` raise after the helper prints a valid acknowledgment. Both
    cases assert `ScreenViewportResizeUnconfirmed`.
  - During the review of that change, a probe wrapped the polling loop of the
    real script in `except Exception: sys.exit(3)` and injected an
    `os.waitid` error. The supervisor exited with status 3, and its helper
    kept running. Every test from before that change passed with that copy.
- **Problem:** `signal.sigtimedwait`, `Popen.wait`, and `os.killpg` have no
  negative test. A later handler that turns one of their errors into a proven
  status passes every test.
- **Impact if no action:** A later change can report a proven failure while
  the helper still runs. The caller then releases its session fence too
  early, and an old resize can still change the viewport.
- **Root question:** How can the tests prove that the real supervisor never
  gives a proven status for an outcome outside the three proven ones?
- **Options:**
  - **A.** Keep the two negative cases. Three calls stay without a negative
    test. Effort: Small.
  - **B.** Add a fault table to the Linux test, with one case for each
    supervisor call that can fail. Effort: Medium.
    - The missing helper path covers `subprocess.Popen`. A fault prologue
      covers each of `signal.sigtimedwait`, `os.waitid`, `Popen.wait`, and
      `os.killpg`.
    - Each case asserts `ScreenViewportResizeUnconfirmed`. Each case also
      asserts that the script reached its injected fault.
    - `Popen.wait` runs in two places: after a helper exit and after the
      group kill. Give each place its own case.
    - The `os.killpg` case needs a helper that outlives the budget, so that
      the script reaches the group kill.
    - Each new supervisor call that can fail needs a new row.
    - `cargo xtask rust-file-length-lint` limits each Rust file to 300 lines
      and has no override. Put the table in the test support module under
      `crates/sandbox-e2b/tests/support/` when the test file gets too long.
- **Left out:**
  - Another mocked status in the unit tests. It never runs the script, and
    the unit tests already send status 1.
  - A live E2B test. It needs credentials and can cost money, and the local
    script reproduces each fault.
  - A prologue that makes `subprocess.Popen` raise. A missing path gives the
    real error at the same cost.
  - A text check of the script for `except` or `sys.exit` lines. It cannot
    prove the behaviour.
- **Wider fix:** B. It is a detection fix. No structural fix was found,
  because any new handler in the script can exit with a proven status.
- **Previous examples:** Commit `8f9a785` of 2026-11-24 added the Linux test
  that runs the real script and checks that timeout cleanup kills the
  helper's children. The item `plans/follow-ups/e2b-resize-cleanup-confirmation.md`
  recorded an earlier case of this class: a signal exit of the supervisor
  proved cleanup. The fix of that case deleted the item, which is still on
  `main` at commit `5d856db`.
- **Related:** `plans/follow-ups/e2b-resize-without-deadline-confirmation.md`.
  If the user chooses its option C, every resize runs the supervisor. This
  table then covers resizes with and without a deadline. Neither item waits
  for the other.
- **Recommendation:** B. It runs the real script for each call that can
  fail, so a handler that proves a fault cannot pass the tests.
- **Action:** ask the user, because it is a test for the class, which needs
  the user's approval.
