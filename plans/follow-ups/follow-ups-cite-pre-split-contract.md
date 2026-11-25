# Follow-up items cite the pre-split contract

- **Severity:** Low. **Effort:** Small. **Confidence:** High. **Harder later:** No.
- **Kind:** Outside the scope.
- **Category:** Process.
- **Added:** 2026-11-25, from the branch
  `calummoore/e2b-confirm-resize-cleanup-and-ci-time-7p51`, PR #17, the
  problems outside the scope of review round 1 and the finding check of
  review finding 5.
- **Location:**
  `plans/follow-ups/docs-miss-decisions-in-force.md:8-11`, `:30`, `:35-225`,
  `:249-255`, `:265-268`, `:309-314`.
  `plans/follow-ups/docs-hold-history-and-provider-rule.md:8-12`, `:32`,
  `:40-53`, `:64-69`, `:85-93`.
  `plans/follow-ups/file-read-requests-debug-prints-paths.md:55-56`,
  `:95-101`, `:107-109`.
  `plans/follow-ups/file-read-results-debug-prints-bytes.md:13`, `:52-54`,
  `:88-96`, `:131-132`.
  `plans/follow-ups/control-http-debug-prints-token-bytes.md:9`, `:49`,
  `:81`, `:100`, `:118`, `:125`.
  `plans/follow-ups/control-client-extends-one-shot-deadline.md:35-40`,
  `:44`, `:157-158`.
- **Reason:** Earlier commits made these fields stale, and the branch that
  found them fixed only the one bullet that it edited.

- **Terms:**
  - **Pre-split contract:** `docs/sandbox-contract.md` before commit
    `9658361`. It had 428 lines and the sections "Bounds And Failure Safety"
    and "Conformance".
  - **Capability pages:** the six pages that the split created:
    `docs/sandbox-file-safety.md`, `docs/sandbox-process-contract.md`,
    `docs/sandbox-terminal-screen-contract.md`,
    `docs/sandbox-image-contract.md`, `docs/sandbox-transport-safety.md`,
    and `docs/sandbox-conformance.md`.
  - **Anchor quote:** a sentence that an item quotes to find its place in a
    doc. A search finds the quote after the lines move.
- **Background:**
  - On 2026-11-05, commit `9658361` of PR #11 split
    `docs/sandbox-contract.md` into a 191-line entry page and the six
    capability pages. Each page has 60 to 100 lines. The contract now has 197
    lines, and neither old section exists.
  - The same commit moved README text that some items cite by line. It
    updated no follow-up item.
  - On 2026-11-24, commit `5d856db` of PR #15 added one line to the section
    "Sandbox Lifetimes" of `docs/e2b-adapter.md`. Each later line of that doc
    moved down by one.
  - The `plans` skill says that a change that edits a protocol section also
    updates each item that names that section.
  - Each stale citation below was checked on 2026-11-25. Each current line
    is from that day.
  - `plans/follow-ups/docs-miss-decisions-in-force.md`:
    - The **Location** line gives ten ranges of `docs/sandbox-contract.md`.
      Seven are past the end of the contract. The other three hold other
      text now.
    - The same line gives `docs/e2b-adapter.md` lines 145-147 and
      `crates/sandbox-e2b/README.md:322-326`. Their anchors are now at
      adapter lines 147-148 and README lines 340-344.
    - Line 30 cites `docs/sandbox-contract.md:88-92`. That rule is now at
      `docs/sandbox-contract.md:144-150`, section "Network Policies".
    - Twelve entries name a home in `docs/sandbox-contract.md`. The list
      below gives the current place of each anchor quote.
    - "Queued data and any separately retained final overflow prefix", for
      the first entry and for "Cleanup keeps queued data":
      `docs/sandbox-process-contract.md:81-83`, section "Streaming Events
      And Completion".
    - "Signal termination reports `exited: false`":
      `docs/sandbox-process-contract.md:56`.
    - "so slow consumers cannot postpone timeout or cleanup.":
      `docs/sandbox-process-contract.md:67-68`.
    - "requires a fetch from a different host to exit unsuccessfully.":
      `docs/sandbox-conformance.md:46-47`, section "Process And Network
      Fixtures".
    - "so conforming images need a standard shell but no harness-only
      executable.": `docs/sandbox-conformance.md:34`.
    - "restored-sandbox cleanup removes all retained terminal identity
      state.", for two entries: `docs/sandbox-terminal-screen-contract.md:56-57`,
      section "Terminal Identity And Delivery".
    - "even when a control transport is injected.": the split reworded it.
      `docs/sandbox-transport-safety.md:10-11`, section "Credentials And
      Routes", now says "including with an injected transport."
    - "Without that usable workload identity, the outcome is unconfirmed.":
      `docs/sandbox-file-safety.md:20-21`, section "Regular Files And
      Replacement".
    - "Image construction is a caller-persisted state machine", step 4:
      `docs/sandbox-image-contract.md:22-35`, section "Durable Image Phases".
    - "cannot set its provider transcript limit above the shared 256 MiB
      regular-file ceiling.": the split reworded it.
      `docs/sandbox-terminal-screen-contract.md:8-9`, section "Terminal
      Bounds And Storage", now says "Terminal create and recovery must not
      accept a transcript limit above the 256 MiB regular-file ceiling."
    - Line 74 cites `crates/sandbox-interface/README.md:137`. That sentence
      is now at line 185.
    - Line 168 cites `crates/sandbox-e2b/README.md` lines 322-326. That
      paragraph is now at lines 340-344. Line 179 cites `docs/e2b-adapter.md`
      lines 146-147. That anchor is now at lines 147-148.
    - The code-name cases of option A cite `docs/sandbox-contract.md:221`,
      `:228-230`, and `:243-261`. `SandboxBackend::stream_process` is in no
      doc now. `validate_execution_context()` is only in
      `docs/juno-adoption.md:113`.
    - The validation order is now at `docs/sandbox-process-contract.md:49-51`.
      The provider reader text is now at
      `docs/sandbox-process-contract.md:65-73`.
    - The same cases cite line 316 for
      `ConnectProcessTransport::stream_process`. It is now at
      `docs/e2b-adapter.md:317`.
    - Option B and two **Related** entries still plan a split of
      `docs/sandbox-contract.md`. That split is done.
    - The **Effort** grade Large comes from option A, which included the
      contract split. Check the grade again.
  - `plans/follow-ups/docs-hold-history-and-provider-rule.md`:
    - Line 51 says that `docs/sandbox-contract.md` has 428 lines and needs a
      split. It has 197 lines.
    - Lines 10, 40, and 66 cite `docs/sandbox-contract.md:197-198` for "For
      E2B, a sandbox ID must fit the lowercase DNS label used for envd." That
      sentence is now at `docs/sandbox-transport-safety.md:22-23`.
    - Lines 47-49 and 66-67 cite the general rule at contract lines 196-197.
      It is now at `docs/sandbox-transport-safety.md:21-22`: "Creation and
      inventory reject returned identities that their later routes cannot
      use."
    - Lines 42-43 quote "any concrete sandbox provider" from contract lines
      3-5. The contract now says "a sandbox provider" at lines 3-4.
    - The **Reason**, option A, the **Related** line, and the **Action** say
      that the fix waits for a split of `docs/sandbox-contract.md`. The fix
      now edits `docs/sandbox-transport-safety.md`, which has 68 lines.
    - Lines 9 and 32 cite `crates/sandbox-interface/README.md:100-102`. That
      passage is now at lines 147-149.
  - `plans/follow-ups/file-read-requests-debug-prints-paths.md`:
    - Lines 55-56 cite `docs/sandbox-contract.md:371-374` for "Process,
      terminal, and file-write request `Debug`". That text is now at
      `docs/sandbox-transport-safety.md:49-51`.
    - Lines 95-97 cite the summary at `docs/sandbox-contract.md:370-386`. It
      is now the section "Diagnostics" of `docs/sandbox-transport-safety.md`,
      lines 46-64. It still states no rule for file-read requests.
    - Lines 98-101 say that an edit of the contract must split it, because
      it has 428 lines. That split is done.
    - Lines 107-109 cite `docs/sandbox-contract.md:373-375` for "instead of
      matching known secrets". That phrase is now at
      `docs/sandbox-transport-safety.md:53`.
  - `plans/follow-ups/file-read-results-debug-prints-bytes.md`:
    - Lines 13 and 52 cite `docs/sandbox-contract.md:385-386`. That sentence
      is now at `docs/sandbox-transport-safety.md:63-64`. Also,
      `docs/sandbox-file-safety.md:60` says "File-read results remain
      sensitive data."
    - Line 89 cites `docs/sandbox-contract.md:370-386`. That text is now at
      `docs/sandbox-transport-safety.md:46-64`.
    - Lines 92-96 say that the change splits the 428-line contract. Lines
      131-132 say that the first change that lands splits it. That split is
      done.
    - Line 53 cites `crates/sandbox-interface/README.md:84`, now line 131.
      Line 90 cites its lines 81-84, now 128-131.
    - Line 54 cites `crates/sandbox-e2b/README.md:224`, now line 236. Line
      91 cites its lines 220-224, now 232-236, and `README.md:106-108`, now
      135-137.
  - `plans/follow-ups/control-http-debug-prints-token-bytes.md`:
    - Line 118 cites `docs/sandbox-contract.md:373-375` for "instead of
      matching known secrets". That phrase is now at
      `docs/sandbox-transport-safety.md:53`.
    - Lines 9, 49, 81, 100, and 125 cite `docs/e2b-adapter.md:174-176`.
      Commit `5d856db` moved that sentence to lines 175-177.
  - `plans/follow-ups/control-client-extends-one-shot-deadline.md`:
    - Lines 35-40 cite the pre-split contract. Its lines 3-5 no longer name
      `sandbox-interface`.
    - "never pauses, is not resumable" is now "never pauses or resumes" at
      `docs/sandbox-contract.md:90`.
    - The access sentence is now "Resume, connect, pause, process execution,
      and other access must not extend its original deadline." at
      `docs/sandbox-contract.md:109-111`.
    - "one-shot sandboxes retain their original destruction deadline." is
      now at `docs/sandbox-process-contract.md:16`.
    - Lines 44 and 157 cite `crates/sandbox-e2b/README.md:79-82`. That text
      is now at lines 91-95.
    - Line 158 stores the line count of `docs/e2b-adapter.md`. The count is
      right on 2026-11-25, but the next edit of that doc makes it stale.
- **Problem:** Six follow-up items cite text of the pre-split contract, and
  some README and adapter text, by line numbers and section names that later
  commits changed. Four of the items still plan a split of the contract that
  is done.
- **Impact if no action:** The next agent on one of these items looks for
  rules at lines that do not hold them. It can quote reworded text or plan
  a split that is done. Each later doc edit moves more lines.
- **Root question:** How do follow-up items give their reader the current
  home of each doc rule that they cite?
- **Options:**
  - **A.** Refresh every stale field of the six items, with the list above.
    Effort: Small.
    - Name the page and the section of each home, and quote its sentence.
    - Store no doc line count. Give the size rule and the command that
      measures it.
    - Keep line numbers only where the item template requires them, such as
      the **Location** line.
  - **B.** Do A, and change the shared follow-up rules in `futex-ai/skills`.
    Effort: Medium.
    - Change `plans/references/follow-up-items.md` and
      `plans/templates/follow-up.md` of that repository.
    - An item cites protocol text by page and by a quoted sentence.
    - An item stores no doc line count. It gives the size rule and the
      command that measures it.
    - The search of "Keeping an item current" also looks for each quoted
      sentence that a change edits or moves.
    - The change is in another repository, and its owner decides it.
  - **C.** Add a Markdown link check for relative links and heading anchors
    in `docs/`, `plans/`, and every README. Items then cite doc homes as
    such links. Effort: Medium.
    - It is a new gate, so it needs the user.
    - It catches a heading that moved. It does not catch a stale count or
      line number.
    - It can need a new crate or tool.
- **Left out:**
  - Change nothing. The items keep wrong homes and plan a split that is
    done, so the goal fails.
  - Refresh each item only when a branch starts its work. The reader then
    meets wrong facts first, and the `plans` skill asks the change that moves
    a section to update the items.
  - Split the shared contract again. Each capability page already has 60 to
    100 lines.
  - A custom checker of plain-text citations. The `testing` skill says that
    plans never get a test, and it allows only a link check, which is C.
- **Wider fix:** B. New items then cite moved text by a quote that a search
  finds, and they store no line count. C is the detection fallback, and it
  needs the user.
- **Previous examples:** Commit `9658361` of 2026-11-05 split the contract
  and updated no follow-up item. Commit `5d856db` of 2026-11-24 moved the
  adapter lines that two of these items cite.
- **Related:**
  - `plans/follow-ups/docs-miss-decisions-in-force.md`: this item refreshes
    its fields. Its option A splits `docs/e2b-adapter.md` and moves its
    lines again, so one branch can apply both items.
  - `plans/follow-ups/docs-hold-history-and-provider-rule.md`,
    `plans/follow-ups/file-read-requests-debug-prints-paths.md`,
    `plans/follow-ups/file-read-results-debug-prints-bytes.md`,
    `plans/follow-ups/control-http-debug-prints-token-bytes.md`, and
    `plans/follow-ups/control-client-extends-one-shot-deadline.md`: this
    item refreshes their fields. Their own work does not wait for it.
- **Recommendation:** A. It corrects the six items in this repository with
  a mechanical edit. B best protects new items, but the owner of
  `futex-ai/skills` decides it.
- **Action:** fix after merge, apply A, because earlier commits made these
  fields stale, outside the diff of the branch that found them.
