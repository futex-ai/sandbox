# Resize can succeed after its execution cutoff

- **Severity:** Low. **Likelihood:** Rare. **Effort:** Medium. **Confidence:** High. **Harder later:** No.
- **Kind:** Fix after merge.
- **Category:** Product bug.
- **Added:** 2026-11-24, from the branch
  `calummoore/e2b-fix-screen-cleanup-test-in-ci-tcpd`, review finding 2.
- **Location:** `crates/sandbox-e2b/src/backend/screen_resize.rs:20-38`.
- **Reason:** The case needs a supervisor stall at the exact expiry, and the
  fix changes the supervisor's wait logic, which costs the same after the
  merge.

- **Terms:**
  - **Supervisor:** the small trusted Python script in `screen_resize.rs`
    that runs the screen helper when the resize request has a deadline.
  - **Execution cutoff:** the time when the helper must stop, so that the
    3-second termination allowance stays for cleanup.
- **Background:** The supervisor calls `Popen.wait(timeout=budget)`. On
  Python 3.12, that wait calls `waitpid(WNOHANG)` before it checks the
  remaining time. GNU `timeout`, the supervisor before 2026-11-24, killed
  the helper's group from its timer instead.
- **Problem:** When the supervisor is stopped across the cutoff and the
  helper exits normally in that time, the supervisor sees the exit first and
  reports success. A local check with a 300 ms budget, a helper that exits
  after 400 ms, and the supervisor stopped from 150 ms to 800 ms gave status
  0 with the acknowledgment in 3 of 3 runs. GNU `timeout` killed its group
  in the same runs. The transport deadline still bounds the whole call.
- **Impact if no action:** In a rare stall, a resize can change the viewport
  in the reserved cleanup window and still report success. Terminal reads
  and the Boat supervisor reject a completion after their cutoff.
- **Root question:** Which result must the caller get when the supervisor
  sees a normal helper exit only after the execution cutoff?
- **Options:**
  - **A.** Check the cutoff before the supervisor accepts a result. Report
    `ScreenViewportResizeUnconfirmed` when the cutoff passed. Effort: Small.
  - **B.** Use one monotonic cutoff from launch through the wait. Observe
    the helper exit before reaping it (for example with a `pidfd`), so that
    expiry can still kill the group while the process identity stays valid.
    A late exit reports the confirmed-timeout status. Effort: Medium.
  - **C.** Give an independent watchdog process the cutoff and the group
    kill. Effort: Large.
  - **D.** Keep the order and record a late normal exit as a success in the
    docs. Effort: Small.
- **Left out:** A longer budget, because the same order problem stays at the
  new cutoff. A kernel timer that the helper inherits, because it does not
  reach the helper's children.
- **Wider fix:** B. One cutoff check governs both launch and completion.
- **Previous examples:** The Boat helper `wait_marker('ack', until)` in
  `crates/sandbox-boat/src/trusted_runtime/helpers/common.py` fails after
  its cutoff.
- **Related:** `plans/follow-ups/e2b-resize-cleanup-confirmation.md` changes
  the same supervisor and its status table. Settle both in one change.
- **Recommendation:** B. It gives resize the same strict cutoff as terminal
  reads and Boat, and it needs no requirement change.
- **Action:** fix after merge, apply B, because it is a Rare product bug that
  is not High, and the fix does not get harder after the merge.
