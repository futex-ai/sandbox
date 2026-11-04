# Preparation expiry reports the wrong outcome

- **Severity:** Medium. **Likelihood:** Rare. **Effort:** Medium. **Confidence:** High. **Harder later:** Yes.
- **Kind:** Needs decision.
- **Added:** 2026-11-04, from PR11 post-push review, finding 1.
- **Location:** `crates/sandbox-boat/src/process/execution.rs:47`.
- **Reason:** The owner explicitly reserved post-review fixes for a later choice. No option is applied.

- **Source:** Review.
- **Category:** Product bug.
- **Terms:**
  - **Preparation:** The Boat setup phase before caller code can start.
- **Background:** Boat process calls have one absolute time budget. An exhausted preparation budget must return a DeadlineExpired stream outcome.
- **Problem:** Preparation can return its own error before the outer timer reports expiry. The backend then returns BackendUnavailable. The related pre-exit release path can also discard deadline or idle outcomes into TransportFailure.
- **Impact if no action:** A caller can treat an elapsed budget as a provider outage. Its retry and status handling can use the wrong reason.
- **Root question:** How should an exhausted preparation budget reach the caller?
- **Options:**
  - **A.** Check absolute expiry before mapping a preparation error. Add the exact regression. The change stays local. Effort: Small.
  - **B.** Expose a typed deadline error through the Boat runtime API. Include clock checks for HTTP timeout ties. This expands the API. Effort: Medium.
  - **C.** Classify expired, completed, and failed steps at the private budget owner. Preserve phase-aware outcomes through the pre-exit release call. No public API changes. Effort: Medium.
  - **D.** Use one shared deadline-race helper for preparation and reader waits. Preserve idle, consumer-drop, and post-exit behavior. Effort: Medium.
- **Left out:** Rejecting short deadlines changes accepted inputs and leaves longer preparation expiry unresolved. Change nothing breaks the existing deadline outcome contract.
- **Wider fix:** C. Classify by the budget clock even when a transport supplies only a generic failure.
- **Previous examples:** None found for this exact classification error. The current code already has a distinct outer timer expiry branch.
- **Recommendation:** C. Keep budget classification at its owner without changing public APIs. The first pass recommended B. B needs additional clock checks to cover generic HTTP timer ties.
- **Action:** Ask the user, because the task explicitly reserves post-review fixes for the owner's choice.
