# BackendUnavailable doc names only a provider outage

- **Severity:** Low. **Effort:** Small. **Confidence:** High. **Harder later:** No.
- **Kind:** Outside the scope.
- **Category:** Docs or spec.
- **Added:** 2026-11-25, from the branch
  `calummoore/e2b-confirm-resize-cleanup-and-ci-time-7p51`, PR #17, a
  problem outside the scope that the finding check of review finding 1
  found.
- **Location:** `crates/sandbox-interface/src/error.rs:287`.
- **Reason:** The branch that found it did not change
  `crates/sandbox-interface/src/error.rs`, and the comment covers every
  adapter, not only resize.

- **Terms:**
  - **`BackendUnavailable`:** a variant of `Error`, the shared error type in
    `crates/sandbox-interface/src/error.rs`. It carries only the configured
    backend ID.
- **Background:**
  - The doc comment at `crates/sandbox-interface/src/error.rs:287` has one
    sentence: "The provider reports that its control plane is temporarily
    unavailable."
  - No protocol doc defines the variant in general. The doc comment is its
    only general definition, and rustdoc shows it to callers.
  - E2B resize returns the variant before it sends a command, when the
    deadline leaves no time to run the helper. The test
    `expired_initialization_never_connects_or_starts_a_helper` at
    `crates/sandbox-e2b/src/backend/_tests_/screen_stack/deadline_tests.rs:20`
    pins that result on `origin/main`.
  - E2B resize also returns the variant for a proven helper failure and for
    a wrong acknowledgment. On 2026-11-24 the user chose that a proven
    resize failure returns `BackendUnavailable`. `docs/e2b-adapter.md` states
    these resize results.
  - Other E2B paths return it too. `crates/sandbox-e2b/src/process/mapping.rs`
    maps envd transport errors to it. `crates/sandbox-e2b/src/backend/screen_stack.rs`
    returns it when screen ensure fails.
  - `crates/sandbox-e2b/src/backend/read_only_exec.rs` returns it when a
    read-only command ends without a normal exit.
- **Problem:** The doc comment names only a temporary control-plane outage.
  Adapters also return the variant for a local deadline, a proven helper
  failure, and process transport errors.
- **Impact if no action:** A caller that reads the comment can treat each
  `BackendUnavailable` as a provider outage and retry. A retry with the same
  spent deadline fails again. The cost is small.
- **Root question:** What must the documentation of `BackendUnavailable`
  tell a caller, so that the caller handles each case in which adapters
  return it?
- **Options:**
  - **A.** Rewrite the doc comment to state the meaning in force. Effort:
    Small.
    - Say that the backend could not complete the operation, and that no
      more specific typed error applies.
    - Name the main causes: a provider outage, a local deadline that leaves
      no time, and a proven helper failure.
    - Change no result. No protocol doc changes.
  - **B.** Give resize a typed error for a deadline that leaves no time, and
    one for a proven helper failure. Keep `BackendUnavailable` for provider
    outages. Effort: Medium.
    - It adds public error variants to `sandbox-interface`, which is new
      behaviour at a boundary. The user decides it.
    - It changes the result that
      `expired_initialization_never_connects_or_starts_a_helper` expects, and
      the result that the user chose on 2026-11-24.
  - **C.** Narrow each use of `BackendUnavailable` in each adapter to a real
    provider outage. Add a typed variant for each other cause. Effort:
    Large.
    - It changes many results on `origin/main` and each caller that matches
      the variant. The user decides it.
- **Left out:**
  - Change nothing. Callers keep a definition that misses most uses.
  - State the resize causes only in `docs/e2b-adapter.md`. That doc already
    states them, and the shared comment stays wrong for the other uses.
- **Wider fix:** C. Each cause then has its own type, so no variant hides a
  different cause. It changes results on `origin/main`, so it needs the
  user.
- **Previous examples:** None found.
- **Related:** `plans/follow-ups/e2b-resize-without-deadline-confirmation.md`.
  Its options B and C change which resize results return
  `BackendUnavailable`. A general comment covers each answer, so neither
  item waits for the other.
- **Recommendation:** A. It aligns the shared definition with the behaviour
  in force, and it changes no result. C best protects callers, but it
  changes results on `origin/main`.
- **Action:** fix after merge, apply A, because the comment is in
  `sandbox-interface`, which the branch that found it did not change.
