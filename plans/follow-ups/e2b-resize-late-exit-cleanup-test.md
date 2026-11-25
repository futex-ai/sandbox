# A late helper exit has no cleanup test

- **Severity:** Low. **Effort:** Medium. **Confidence:** High. **Harder later:** No.
- **Kind:** Fix after merge.
- **Category:** Test.
- **Added:** 2026-11-25, from the branch
  `calummoore/e2b-confirm-resize-cleanup-and-ci-time-7p51`, PR #17, the
  wider fix of review finding 7.
- **Location:** `crates/sandbox-e2b/src/backend/screen_resize.rs:85`,
  `crates/sandbox-e2b/tests/screen_resize_deadline.rs`.
- **Reason:** The branch that found it had used its last review fix round,
  and the case needs a new fault prologue.

- **Terms:**
  - **Resize helper:** the template program at the configured screen-helper
    path that changes the sandbox viewport.
  - **Supervisor:** the trusted Python script `RESIZE_SUPERVISOR` in
    `crates/sandbox-e2b/src/backend/screen_resize.rs`. It runs the resize
    helper when the resize request has a deadline.
  - **Cutoff:** the end of the time budget that the supervisor gives the
    helper.
  - **Child fixture:** a process that the Linux test helper starts in the
    helper's process group. It writes a readiness marker, waits for the
    fixture release on stdin, and then writes a late marker.
- **Background:**
  - The supervisor polls for the helper exit. When it sees the exit before
    the cutoff, it reaps the helper and exits with status 0 or 3. It does
    not stop the helper's process group, so a process that the template
    helper leaves running on purpose continues. The user chose this on
    2026-11-24, when option C of the deleted item
    `plans/follow-ups/e2b-resize-cleanup-confirmation.md` was not applied.
  - When the supervisor sees the exit only at or after the cutoff, it kills
    the helper's group, reaps the helper, and exits with status 124. The
    adapter then returns `BackendUnavailable`. Commit `8f9a785` chose this
    rule. The section "Files, Processes, Terminals, And Screens" of
    `docs/e2b-adapter.md` states it.
  - The Linux test `crates/sandbox-e2b/tests/screen_resize_deadline.rs`
    runs the real script with `/usr/bin/python3`. Since 2026-11-25 its
    shared child fixture shows that the child survives an exit that the
    supervisor sees before the cutoff, and that timeout and signal cleanup
    kill the child.
- **Problem:** No test reaches the cutoff check after the supervisor sees
  the helper exit. During the review of PR #17, a probe removed that check.
  Every test still passed. Only an exit seen late changed, from status 124
  and a killed child to status 0 and a live child.
- **Impact if no action:** A later change can remove or move the check. The
  adapter then reports success for a helper that ended after its budget,
  and it leaves the helper's children running after the deadline.
- **Root question:** How can the tests prove that a helper exit that the
  supervisor sees only after the cutoff gives cleanup and a proven failure?
- **Options:**
  - **A.** Add one row to the Linux test that uses the shared child fixture.
    A fault prologue wraps `os.waitid`. It moves the test clock past the
    cutoff when the call reports the helper exit. The row asserts status
    124, `BackendUnavailable`, and that the child's late marker does not
    exist. Effort: Medium.
  - **B.** Keep the current tests. The check stays without a test.
    Effort: Small.
- **Left out:** A unit test with a mocked status, because it never runs the
  script. A live E2B test, because it needs credentials and the local
  script reproduces the case. A text check of the script, because it cannot
  prove the behaviour.
- **Wider fix:** None. A is the regression test for the documented rule.
  `plans/follow-ups/e2b-resize-supervisor-fault-table.md` holds the wider
  fault table, which needs the user.
- **Previous examples:** Commit `8f9a785` added the readiness and release
  events that the Linux test uses to prove that timeout cleanup kills a
  child.
- **Related:** `plans/follow-ups/e2b-resize-supervisor-fault-table.md`: both
  add fault prologues to the same Linux test. Its `os.killpg` row can reuse
  the child fixture. Neither item waits for the other.
- **Recommendation:** A. It pins the documented cleanup rule with one row
  and the existing fixture.
- **Action:** fix after merge, apply A, because the branch that found it
  used its last review fix round.
