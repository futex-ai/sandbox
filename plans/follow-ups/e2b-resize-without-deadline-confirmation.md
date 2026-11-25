# Resize without a deadline confirms signal ends

- **Severity:** High. **Likelihood:** Rare. **Effort:** Medium. **Confidence:** Medium. **Harder later:** No.
- **Kind:** Needs decision.
- **Category:** Product bug.
- **Added:** 2026-11-25, from the branch
  `calummoore/e2b-confirm-resize-cleanup-and-ci-time-7p51`, PR #17, the
  finding check of review finding 3.
- **Location:** `crates/sandbox-e2b/src/backend/screen_resize.rs:143`,
  `crates/sandbox-e2b/src/backend/_tests_/screen_stack/resize_tests.rs:134`.
- **Reason:** The user's decision of 2026-11-24 covered only a resize with
  a deadline, and each fix except option A changes a tested result on
  `origin/main`.

- **Terms:**
  - **Resize helper:** the template program at the configured screen-helper
    path that changes the sandbox viewport.
  - **Supervisor:** the trusted Python script in
    `crates/sandbox-e2b/src/backend/screen_resize.rs` that runs the resize
    helper when a resize has a deadline.
  - **envd:** the E2B service in the sandbox that runs processes and reports
    their end to the adapter.
  - **Session fence:** the guard that the caller keeps while an old resize
    can still run. `ScreenViewportResizeUnconfirmed` tells the caller to keep
    it.
- **Background:**
  - The section "Screens" of `docs/sandbox-terminal-screen-contract.md`
    says: "An adapter that cannot confirm resize process termination returns
    `ScreenViewportResizeUnconfirmed`; the caller must retain its session
    fence and arrange cleanup."
  - A resize request has an optional absolute deadline, the field
    `deadline_at` of `BackendResizeScreenStackRequest`. Bounded screen
    initialization sends a deadline. Other resizes can omit it.
  - With a deadline, the supervisor runs the helper. On 2026-11-24 the user
    chose the rule for this mode. Only three normal supervisor exits prove
    that the helper ended: helper success, helper failure, and a kill of the
    helper's process group.
  - In that mode, a proven failure returns `BackendUnavailable`. Every other
    outcome returns `ScreenViewportResizeUnconfirmed`. This includes a signal
    end of the supervisor. `docs/e2b-adapter.md` states this rule.
  - Without a deadline, the adapter runs the helper directly through envd,
    with a 15-second transport deadline. A normal exit with status zero and
    the exact acknowledgment gives success.
  - In that mode, every other reported exit gives `BackendUnavailable`. This
    includes a signal end, which envd reports with `exited: false`. No
    reported end, or a transport error, gives
    `ScreenViewportResizeUnconfirmed`.
  - The test `resize_rejects_acknowledgment_from_a_signalled_helper` at
    `crates/sandbox-e2b/src/backend/_tests_/screen_stack/resize_tests.rs:134`
    pins the signal result. It sends `exited: false`, `exit_code: Some(0)`,
    a valid acknowledgment, and no deadline. It expects `BackendUnavailable`.
    Commit `c96c308` added it on 2026-09-22.
  - envd normally sends its end event only after end-of-file on the stdout
    and stderr of the process. The helper's children can hold those pipes.
    envd's own `SendSignal` call cancels that wait.
  - So when envd's `SendSignal` kills the helper, envd reports a signal end
    while a child of the helper can still run. The adapter returns
    `BackendUnavailable`, and the caller releases its fence.
  - The transport itself sends `SIGKILL` to the helper's PID only at its own
    deadline. Then `run_split` fails, so that case returns
    `ScreenViewportResizeUnconfirmed`.
  - A resize with a deadline had the same gap until 2026-11-24. The decision
    of that day named only that mode, so the fix did not change this mode.
- **Problem:** A resize without a deadline reports a signal end of the
  helper as the proven failure `BackendUnavailable`. A signal end does not
  prove that the helper's children ended.
- **Impact if no action:** In rare cases, the caller releases its session
  fence while an old resize process can still change the viewport.
- **Root question:** What evidence must the adapter have before it reports a
  resize without a deadline as a proven failure?
- **Options:**
  - **A.** Accept the gap. Keep the current results, and record the gap as
    accepted in the resize paragraph of `docs/e2b-adapter.md`. Effort: Small.
  - **B.** Map a signal end, `exited: false`, of a resize without a deadline
    to `ScreenViewportResizeUnconfirmed`. Effort: Small.
    - Keep a normal non-zero exit as `BackendUnavailable`.
    - Change the expected result of
      `resize_rejects_acknowledgment_from_a_signalled_helper` to
      `ScreenViewportResizeUnconfirmed`.
    - Update the resize paragraph of `docs/e2b-adapter.md`.
    - A hung helper still gives `ScreenViewportResizeUnconfirmed`. Resize
      keeps two modes with two status rules.
  - **C.** Run every resize under the supervisor. Use the 15-second budget
    when the request has no deadline. One status table then covers both
    modes. Effort: Medium.
    - The command of a resize without a deadline changes. The test
      `resize_forwards_and_requires_the_exact_acknowledgment` at
      `resize_tests.rs:16` expects the direct helper command today.
    - Two results change. A hung helper gives `BackendUnavailable` after the
      group kill, not `ScreenViewportResizeUnconfirmed`. An outside kill of
      the started process gives `ScreenViewportResizeUnconfirmed`, not
      `BackendUnavailable`.
    - Each resize starts `/usr/bin/python3`. The trusted file, terminal, and
      image helpers already need it, so the template needs nothing new.
    - Update the resize paragraph of `docs/e2b-adapter.md` and the screen
      paragraph of `crates/sandbox-e2b/README.md`.
- **Left out:**
  - Use envd's end-of-file wait as the evidence. envd skips that wait after
    its own `SendSignal`.
  - Kill the helper's process group after each helper exit. That kill also
    stops a process that the template helper leaves running on purpose. The
    rule of 2026-11-24 keeps such a process.
  - Run a second trusted command after an unproven outcome, to find and kill
    the helper's processes. On 2026-11-24 the user chose the status table
    over this approach for the same gap with a deadline. Its effort is Large.
- **Wider fix:** C. One status table then decides every resize, so no mode
  can prove an end without evidence.
- **Previous examples:** The item
  `plans/follow-ups/e2b-resize-cleanup-confirmation.md` recorded the same gap
  for a resize with a deadline. The fix of that gap deleted the item, which
  is still on `main` at commit `5d856db`. GNU `timeout`, the earlier
  supervisor, had the same gap.
- **Related:**
  - `plans/follow-ups/e2b-split-output-normal-exit-helper.md`: its option D
    is option C of this item. If the user chooses C, apply this item first,
    because resize then has one mode.
  - `plans/follow-ups/e2b-resize-supervisor-fault-table.md`: with C, that
    fault table also covers resizes without a deadline.
- **Recommendation:** C. It gives every resize one rule, so only the three
  supervisor outcomes prove that the helper ended. B is the smallest safe
  step: one match arm and one test change remove the false proof, without a
  new command or a Python start.
- **Action:** ask the user, because every option except A changes the
  expected result of a test on `origin/main`, which is live functionality.
