# E2B screen cleanup fails in CI

- **Severity:** Medium. **Effort:** Medium. **Confidence:** High. **Harder later:** No.
- **Kind:** Outside the scope.
- **Category:** Test.
- **Added:** 2026-11-17, from `fix/boat-file-ack-paths` and PR #14.
- **Location:** `crates/sandbox-e2b/tests/screen_resize_deadline.rs:99`,
  `crates/sandbox-e2b/src/backend/screen_resize.rs`.
- **Reason:** The failure also exists on `origin/main`, and the user limited
  this change to one Boat file-acknowledgment fix.

- **Terms:**
  - **Screen cleanup test:** The E2B integration test that runs an actual
    timeout supervisor around a Python helper and its child.
- **Background:** The E2B adapter must stop an unfinished screen helper and
  its children when initialization times out. The test is
  `initialization_timeout_kills_the_helper_and_its_children_before_returning`.
  It supplies a four-second deadline. Its child writes a file after a
  1.5-second sleep. The test waits 1.6 seconds after the call returns and
  requires that file to be absent.
- **Problem:** GitHub's Ubuntu CI reaches the final assertion with that file
  present. It reports `a child escaped the supervisor's process group` at
  `crates/sandbox-e2b/tests/screen_resize_deadline.rs:99`.
  - The push CI of the current main commit
    `037fbd7e3671975e79775c7dbcfb95b1a1fe37ab` failed with this assertion.
  - The pull-request CI of Boat fix commit
    `f61d6313547508212a37283d7995839ef2f42736` failed with the same assertion.
  - One rerun of that failed job also failed with the same assertion.
  - The complete local gate on the Boat fix passed with 515 tests passed,
    zero failed, and four live E2B tests ignored.
  - The suspected source is the interaction between real deadlines, Python
    subprocess scheduling, and supervisor termination. The cause is not
    confirmed. The CI evidence does not distinguish a fixture defect from
    a supervisor defect.
- **Impact if no action:** CI remains red for otherwise independent changes.
  The failed test prevents later file-length, smoke, and full-gate steps.
- **Root question:** How should the existing E2B failure be handled without
  expanding the approved Boat fix?
- **Options:**
  - **A.** Reproduce and diagnose the failure in a separate E2B change.
    Correct its cause while preserving the cleanup requirement and every
    existing assertion. Effort: Medium.
- **Left out:** Fixing E2B in this branch exceeds the user's one-fix scope.
  Skipping the test, weakening its assertions, or increasing its time limits
  requires a separate user decision. These approaches do not preserve the
  current verification requirement.
- **Wider fix:** None until diagnosis identifies whether cleanup or the
  fixture causes the failure. The shared testing skill already requires
  deterministic tests that keep their correctness assertions.
- **Previous examples:** Main CI run `44067487395` failed on 2026-11-17.
  Pull-request CI run `44128448223` failed on both its first and second
  attempts on the same date.
- **Recommendation:** A. A separate E2B change preserves the approved scope
  and can verify the real cause before selecting a fix.
- **Action:** fix after merge, apply A, because the failure exists on main
  and the user requested only the Boat fix in this branch.

Reproduce without provider credentials:

```bash
cargo test --locked -p sandbox-e2b --test screen_resize_deadline
```

The two CI runs retain the original evidence:
[main](https://github.com/futex-ai/sandbox/actions/runs/44067487395)
and [Boat fix](https://github.com/futex-ai/sandbox/actions/runs/44128448223).
