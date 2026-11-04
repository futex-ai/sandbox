# An unopened reader can leave temporary files

- **Severity:** Medium. **Likelihood:** Common. **Effort:** Medium. **Confidence:** High. **Harder later:** Yes.
- **Kind:** Needs decision.
- **Added:** 2026-11-04, from PR11 post-push review, finding 2.
- **Location:** `crates/sandbox-boat/src/process/reader.rs:53`.
- **Reason:** The owner explicitly reserved post-review fixes for a later choice. No option is applied.

- **Source:** Review.
- **Category:** Product bug.
- **Terms:**
  - **Reader:** The background task that opens and consumes one Boat process stream.
  - **PID:** An operating-system process identifier.
  - **Launch release:** The marker that permits the trusted helper to start caller code.
- **Background:** Preparation creates private temporary files before a stream is returned. A known unopened reader can remove them without killing any process. The no-provider-call wording needs clarification for cleanup-only helper calls.
- **Problem:** One stream poll transfers cleanup ownership to the reader. Dropping before that task runs takes the no-PID early return and skips cleanup. The same cause affects a drop after opening but before launch release.
- **Impact if no action:** Repeated startup cancellations leave private staging and state directories until VM destruction. Temporary disk use grows unnecessarily.
- **Root question:** How should prepared state be cleaned after startup cancellation?
- **Options:**
  - **A.** Clean the known unopened attempt when the reader was never polled. Keep the unknown opened-process path unchanged. Add the exact regression. Effort: Small.
  - **B.** Keep an explicit unopened-attempt cleanup guard through the reader handoff. Disarm it only when opening is actually polled. This changes ownership code. Effort: Medium.
  - **C.** Track whether launch release was requested. Clean every earlier drop with bounded helpers. Preserve unknown-PID state after release. Effort: Small.
  - **D.** Carry one cleanup lease through the reader handoff until launch release. This covers both startup cancellation windows and early returns. Effort: Medium.
  - **E.** Add stale-attempt reclamation to a direct fix. This also covers adapter restarts but adds trusted deletion rules. Effort: Large.
- **Left out:** Always cleaning unknown-PID attempts after release can remove live state. Caller timing restrictions leave the library defect. Change nothing breaks cleanup requirements. A and B cover the reported one-poll window only. Cleanup options must clarify that bounded cleanup-only helper calls before open are allowed. Before open, cleanup must not start caller code. After release, the existing unknown-PID no-kill rule stays.
- **Wider fix:** D. Keep ownership until caller work can start, rather than only until the open poll.
- **Previous examples:** `PreparationGuard` in `crates/sandbox-boat/src/trusted_runtime/attempt.rs` and `LaunchGuard` in `crates/sandbox-boat/src/process/stream.rs` handle adjacent cancellation boundaries.
- **Recommendation:** D. Keep cleanup attached until launch release so both startup cancellation windows are covered. The first pass recommended B.
- **Action:** Ask the user, because the task explicitly reserves post-review fixes for the owner's choice.
