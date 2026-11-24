# Resize can confirm cleanup while helper processes live

- **Severity:** High. **Likelihood:** Rare. **Effort:** Medium. **Confidence:** Medium. **Harder later:** No.
- **Kind:** Needs decision.
- **Category:** Product bug.
- **Added:** 2026-11-24, from the branch
  `calummoore/e2b-fix-screen-cleanup-test-in-ci-tcpd`, review finding 1 and
  two problems that its finding checks found.
- **Location:** `crates/sandbox-e2b/src/backend/screen_resize.rs:20-38`,
  `crates/sandbox-e2b/src/backend/screen_resize.rs:83`,
  `crates/sandbox-e2b/src/backend/_tests_/screen_stack/deadline_tests.rs:97-114`.
- **Reason:** The same gap existed with GNU `timeout` on `origin/main`, and
  every fix reverses the expected result of a unit test on `origin/main`, so
  the user decides.

- **Terms:**
  - **Resize helper:** the template program at the configured
    `E2bRuntimeConventions` screen-helper path that changes the sandbox
    viewport.
  - **Supervisor:** the small trusted Python script in `screen_resize.rs`
    that runs the helper when the resize request has a deadline. It starts
    the helper in a new session, and at expiry it sends `SIGKILL` to the
    helper's process group, reaps the helper, and exits with status 124.
  - **envd:** the E2B service in the sandbox that runs processes and reports
    their exit to the adapter.
  - **Session fence:** the guard that the caller holds while a resize may
    still be active. `docs/sandbox-terminal-screen-contract.md` says that the
    adapter returns `ScreenViewportResizeUnconfirmed` when it cannot confirm
    resize process termination, and the caller then keeps its fence.
- **Background:** Before 2026-11-24 the adapter used
  `/usr/bin/timeout --signal=KILL` as the supervisor. GNU `timeout` kills the
  helper's group and then itself on expiry, so its expiry path ended in a
  signal. The unit test `a_supervisor_exit_confirms_deadline_cleanup` encodes
  that design: envd reports `exited: false, exit_code: -1`, and the adapter
  returns the confirmed failure `BackendUnavailable`. The Python supervisor
  replaced GNU `timeout`, because uutils `timeout` on Ubuntu 26.04 does not
  kill the group. The adapter still maps every supervisor exit code to a
  confirmed result.
- **Problem:** Three cases report a confirmed result while a helper process
  can still run:
  - Something outside the adapter sends `SIGKILL` to the supervisor. The
    helper is in its own session and keeps running. envd reports a signal
    exit, and the adapter returns `BackendUnavailable`. GNU `timeout` on
    `origin/main` had the same gap, because a `SIGKILL` to its PID left the
    helper alive in its group. The transport itself sends `SIGKILL` to one
    PID only at its own deadline, and then `run_split` fails, so the adapter
    returns `ScreenViewportResizeUnconfirmed` in that case.
  - The supervisor gets `SIGTERM`, `SIGINT`, `SIGHUP`, or `SIGQUIT`. It has
    no handler, so it dies and the helper keeps running. GNU `timeout`
    forwarded these signals to the helper's group and waited for the
    helper. No adapter code sends these signals.
  - The helper exits, normally or with an error, before the budget ends, and
    leaves a child running. The supervisor exits at once without a group
    kill. GNU `timeout` behaved the same.
- **Impact if no action:** In these rare cases the caller can release its
  session fence while an old resize process can still change the viewport.
- **Root question:** What evidence must the adapter have before it reports a
  deadline-bound resize as confirmed, success or failure?
- **Options:**
  - **A.** Keep the current mapping. Record the gap as accepted. Effort:
    Small.
  - **B.** Give the supervisor an explicit status table: helper success,
    confirmed timeout cleanup (124), and helper failure, with statuses that a
    Python error exit cannot give (not 1). Map a signal exit and every other
    status to `ScreenViewportResizeUnconfirmed`. Change the expected result
    of `a_supervisor_exit_confirms_deadline_cleanup` to
    `ScreenViewportResizeUnconfirmed`, and add a unit test that status 124
    gives `BackendUnavailable`. Effort: Medium.
  - **C.** Do B, and add handlers for `SIGTERM`, `SIGINT`, `SIGHUP`, and
    `SIGQUIT` that kill the helper's group, reap the helper, and exit with
    the confirmed-cleanup status. The handler must leave the interrupted
    `Popen.wait` before it reaps, because that wait can hold Popen's lock.
    Effort: Medium.
  - **D.** Do C, and also kill the helper's group after the helper exits
    before the budget ends. Risk: the kill also stops a process that the
    template helper leaves on purpose. Effort: Medium.
  - **E.** After an outcome that is not on the allow-list, run a second
    trusted command in the termination allowance. It finds the helper's
    processes, kills them, and confirms that none remains. Effort: Large.
- **Left out:** Using envd's end-of-file wait as the evidence, because envd
  skips that wait after its own `SIGKILL` or `SIGTERM`. Rebuilding the GNU
  self-kill design, because a self-kill looks the same as an outside
  `SIGKILL`.
- **Wider fix:** C. The allow-list removes the false confirmation for every
  unexpected supervisor outcome.
- **Previous examples:** The Boat supervisor requires child exit, trusted
  completion, and a normal supervisor exit as separate evidence
  (`docs/boat-adapter.md`).
- **Related:** `plans/follow-ups/e2b-resize-late-success-after-cutoff.md`
  changes the same supervisor wait. Settle both in one change, so that the
  status table has a value for a completion seen after the cutoff.
- **Recommendation:** C. It requires affirmative evidence before a confirmed
  result, and it restores the signal forwarding that GNU `timeout` gave.
- **Action:** ask the user, because options B to E reverse the expected
  result of the `origin/main` unit test
  `a_supervisor_exit_confirms_deadline_cleanup`, which is live functionality.
