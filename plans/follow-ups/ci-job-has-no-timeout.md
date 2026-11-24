# The CI job has no time limit

- **Severity:** Low. **Effort:** Small. **Confidence:** High. **Harder later:** No.
- **Kind:** Needs decision.
- **Category:** Process.
- **Added:** 2026-11-24, from the branch
  `calummoore/e2b-fix-screen-cleanup-test-in-ci-tcpd`, a finding check of
  review finding 6.
- **Location:** `.github/workflows/ci.yml:13`.
- **Reason:** The branch that found it must not change the CI workflow
  without a decision, and a new job time limit is a new gate.

- **Terms:**
  - **Job time limit:** the `timeout-minutes` key of a GitHub Actions job.
    Without it, GitHub stops a job only after 360 minutes.
- **Background:** The `Rust` job in `.github/workflows/ci.yml` runs the
  complete gate. A normal run takes a few minutes.
- **Problem:** The job sets no `timeout-minutes`. A test that hangs holds
  the runner for up to 360 minutes before CI reports a failure.
- **Impact if no action:** A hung test costs up to six hours of runner time
  and delays the red result.
- **Root question:** How long may one CI run of the gate take before CI
  reports a failure?
- **Options:**
  - **A.** Add `timeout-minutes: 30` to the `Rust` job. Effort: Small.
  - **B.** Keep the GitHub default, and rely on hang guards in each test.
    Effort: Small.
- **Left out:** A limit on each step, because one job limit covers every
  step.
- **Wider fix:** None. The `testing` skill already asks for hang guards in
  tests.
- **Previous examples:** None found.
- **Related:** None.
- **Recommendation:** A. It bounds the cost of any future hang at a value
  far above a normal run.
- **Action:** ask the user, because the option adds a new CI limit, and the
  user decides each new gate.
