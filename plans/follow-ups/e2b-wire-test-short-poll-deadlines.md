# Wire tests poll with four-second deadlines

- **Severity:** Low. **Effort:** Medium. **Confidence:** Medium. **Harder later:** No.
- **Kind:** Outside the scope.
- **Category:** Test.
- **Added:** 2026-11-24, from the branch
  `calummoore/e2b-fix-screen-cleanup-test-in-ci-tcpd`, a finding check of
  review finding 3.
- **Location:** `crates/sandbox-e2b/src/process/_tests_/wire_tests.rs:52`,
  `crates/sandbox-e2b/src/process/_tests_/wire_tests.rs:234`.
- **Reason:** The branch that found it fixes only the screen cleanup test,
  and it does not touch this file.

- **Terms:**
  - **Polling deadline:** the time limit of a loop that waits for an
    expected state, such as a file that appears.
- **Background:** The shared `testing` skill allows polling deadlines that
  wait for an expected state, and it asks for at least 10 seconds, so that a
  slow machine does not fail the test.
- **Problem:** `wait_for_transcript_size` and `wait_for_path` in
  `wire_tests.rs` stop after 4 seconds. Nobody has seen them fail.
- **Impact if no action:** A slow or loaded CI worker can fail these tests
  while the product works.
- **Root question:** How can these waits stop a hung test without a speed
  contract?
- **Options:**
  - **A.** Raise both polling deadlines to 10 seconds. The tests return as
    soon as the state appears, so a normal run does not get slower. This
    option raises a test time limit, so only the user can choose it.
    Effort: Small.
  - **B.** Replace the polls with an explicit readiness signal from the
    terminal wrapper, such as a pipe that it writes. Effort: Medium.
- **Left out:** A retry, because it hides the cause.
- **Wider fix:** None. The `testing` skill rule is in force.
- **Previous examples:** None found.
- **Related:** None.
- **Recommendation:** B. It removes the speed dependence, and it raises no
  time limit.
- **Action:** fix after merge, apply B, because the file is outside the
  branch that found it.
