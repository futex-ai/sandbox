# Docs miss sixteen decisions in force

- **Severity:** Medium. **Effort:** Large. **Confidence:** High. **Harder later:** No.
- **Kind:** Fix after merge.
- **Category:** Docs or spec.
- **Added:** 2026-11-04, from the branch `conductor/agents-md-sandbox`. That
  branch deleted the eight completed plans that recorded these decisions.
- **Location:** `docs/sandbox-contract.md` at lines 123-127, 150-151,
  188-191, 211-212, 234, 245-246, 259-261, 335-338, 394-397, and 409-410.
  `docs/e2b-adapter.md` at lines 24-27 and 145-147.
  `docs/process-diagnostics.md:34-36`. `crates/sandbox-e2b/README.md:322-326`.
- **Reason:** A design call of 2026-11-04 kept doc splits out of the branch
  that deleted the plans, and this fix needs them.

- **Terms:**
  - **Decision in force:** a recorded choice that still applies, which the
    `documentation` skill from `futex-ai/skills` keeps next to its requirement.
- **Background:**
  - The `documentation` skill says that a decision in force gives "the
    choice, the rejected alternative, and the reason". Its purpose is "to
    stop an agent from proposing the rejected alternative again".
  - Each deleted plan was the dated record of its decisions. The permalinks
    below keep each record at commit `6f4b0e540703ca5657a3bd28dd917a4263cdd0c0`.
  - An analysis of the eight plans found sixteen decisions in force that the
    docs do not hold. The first two entries below are requirements. The other
    fourteen are rejected alternatives that the docs neither state nor rule
    out. A later commit changed the decision of the first entry only.
  - The code already has each behaviour below, and a recorded decision chose
    each one. No entry needs a new user decision.
  - `docs/sandbox-contract.md:88-92` already holds one more candidate: an
    adapter rejects a policy variant that it cannot apply. Do not add it.
  - Line numbers are from the day that this item was added. Each entry also
    quotes the text at its home, so that a split does not hide the place.
  - A stalled kill cannot delay the stream outcome.
    - Home: `docs/sandbox-contract.md`, section "Bounds And Failure Safety",
      the streaming paragraph. Replace the sentence at lines 259-261 that
      starts "Queued data and any separately retained final overflow prefix".
    - Text: "A consumer that keeps reading receives the queued data and any
      separately retained final overflow prefix, then the terminal outcome
      and EOF. A stalled kill cannot delay the outcome. Cleanup does not wait
      for consumer capacity, so a consumer that stops reading delays only its
      own reads. Holding the terminal outcome until cleanup ends was rejected,
      because a stalled kill could then delay the outcome past the absolute
      deadline."
    - Record: `plans/stream-process-runs.md`, lines 93-101 and 152-167.
    - History: commit `7d6e703` added the first sentence of this rule to the
      contract on 2026-09-22. Commit `f010faf` replaced it the same day, with
      the choice of the maintainer. Its text ended: "A slow consumer can delay
      its own observation but cannot extend process execution or cleanup."
      The merge commit `fa11e0e` of 2026-09-23 dropped the `f010faf` version.
      It kept only the order of the queued data and the rule that cleanup
      does not wait for consumer capacity.
    - `crates/sandbox-e2b/src/process/stream_run.rs:111-119` stores the
      outcome before it waits for the kill. The test
      `deadline_outcome_and_eof_precede_stalled_kill_cleanup` at
      `crates/sandbox-e2b/src/process/_tests_/stream_run/cleanup_tests.rs:86`
      checks that a stalled kill does not delay the outcome and EOF. The test
      `full_event_queue_does_not_block_cleanup_or_terminal_outcome` at
      `cleanup_tests.rs:134` pins the decision in force. With a full queue,
      the kill still starts, and the queued data comes before the outcome.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/stream-process-runs.md?plain=1#L93-L101>
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/stream-process-runs.md?plain=1#L152-L167>
  - Any HTTP response from the other host fails the network probe.
    - Home: `docs/sandbox-contract.md`, section "Conformance". Add it after
      "requires a fetch from a different host to exit unsuccessfully." at
      lines 409-410.
    - Text: "Any HTTP response from the other host fails the probe. An HTTP
      error status is also a response, so it does not prove a denial."
    - Record: `plans/egress-allowlist-policy.md`, lines 63-71 and 83-84. The
      user approved the fix of all three review findings. Commit `1792b0e`
      applied it on 2026-09-22.
    - The current text allows a curl command with `--fail`, which exits
      unsuccessfully on an HTTP error. Commit `1792b0e` fixed that bug. Only
      `crates/sandbox-interface/README.md:137` states the rule: "verifies
      that another domain cannot return an application response".
    - `crates/sandbox-interface/src/conformance_network.rs:13` and `:46-55`
      fail the probe when curl exits with 0. The test
      `disallowed_fetch_does_not_turn_http_errors_into_denial_success` is at
      `crates/sandbox-interface/src/_tests_/conformance_network_tests.rs:6`.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/egress-allowlist-policy.md?plain=1#L63-L84>
  - No tombstone process and no create acknowledgment.
    - Home: `docs/sandbox-contract.md`, section "Bounds And Failure Safety",
      the terminal identity paragraph. Add it after "restored-sandbox
      cleanup removes all retained terminal identity state." at lines
      337-338.
    - Text: "The provider does not keep a finished terminal process alive as
      a tombstone. Callers do not acknowledge a terminal create. The durable
      identity record makes both unnecessary, because it survives a normal
      exit."
    - Record: `plans/harden-sandbox-and-terminal-create-recovery.md`, lines
      57-72. Commit `afb3b69` recorded it, and commit `8caeb78` applied it,
      both on 2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/harden-sandbox-and-terminal-create-recovery.md?plain=1#L57-L72>
  - Close keeps the identity record instead of draining the transcript.
    - Home: the same paragraph, after the same sentence.
    - Text: "Close does not drain or persist the transcript before it
      returns. Later cursor reads return the final bytes through the kept
      identity record. A drain at close was rejected, because it adds an
      ingestion path beside the cursor-based read API."
    - Record: `plans/harden-sandbox-and-terminal-create-recovery.md`, lines
      303-314. Commit `b832b25` recorded and applied it on 2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/harden-sandbox-and-terminal-create-recovery.md?plain=1#L303-L314>
  - A returned routing domain is ignored, not compared.
    - Home: `docs/sandbox-contract.md`, section "Bounds And Failure Safety".
      Add it after "even when a control transport is injected." at lines
      190-191.
    - Text: "A returned domain that differs from configuration does not fail
      the call. The adapter ignores it, because configuration is the trusted
      routing authority."
    - Record: `plans/harden-stream-completion-and-credential-routing.md`,
      lines 34-44. Commit `b59399f` recorded and applied it on 2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/harden-stream-completion-and-credential-routing.md?plain=1#L34-L44>
  - One probe process also checks stderr.
    - Home: `docs/sandbox-contract.md`, section "Conformance". Add it after
      "so conforming images need a standard shell but no harness-only
      executable." at lines 396-397.
    - Text: "One process checks the working directory, the environment,
      stdout, and stderr. A second probe for stderr only was rejected,
      because one provider process covers all four."
    - Record: `plans/process-run-cwd-and-env.md`, lines 78-86. The user
      selected this choice. Commit `3b75f1b` recorded it on 2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/process-run-cwd-and-env.md?plain=1#L78-L86>
  - The exit event keeps a flag instead of a separate signal event.
    - Home: `docs/sandbox-contract.md`, the streaming paragraph. Add it after
      "Signal termination reports `exited: false`" at line 234.
    - Text: "A separate signal-termination event was rejected. The `exited`
      flag keeps the status that collected runs already report."
    - Record: `plans/stream-process-runs.md`, lines 84-92. Commit `99ae681`
      recorded it, and commit `7d6e703` applied the choice, both on
      2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/stream-process-runs.md?plain=1#L84-L92>
  - Cleanup keeps queued data.
    - Home: the same place as the first entry. Add it after the first
      entry's text "a consumer that stops reading delays only its own reads."
    - Text: "Cleanup does not discard data that is already in the consumer
      queue. A consumer-lag outcome that drops that data was rejected,
      because a consumer that resumes polling must receive it in order."
    - Record: `plans/stream-process-runs.md`, lines 152-167. Commit `417cb5a`
      recorded it, and commit `f010faf` applied the choice, both on
      2026-09-22. The maintainer made the choice.
    - On 2026-09-23 commit `a12b098` added `ConsumerBackpressure`, an outcome
      for a full staging buffer. That outcome keeps the queued data, so this
      rejection still holds.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/stream-process-runs.md?plain=1#L152-L167>
  - A full consumer queue does not end the stream.
    - Home: `docs/sandbox-contract.md`, the streaming paragraph. Add it after
      "so slow consumers cannot postpone timeout or cleanup." at lines
      245-246.
    - Text: "The stream does not end only because the 16-event consumer
      queue is full. That option was rejected, because it loses more output
      and changes the behaviour for slow consumers."
    - Record: `plans/stream-process-runs.md`, lines 328-342. Commit `5d8915d`
      recorded it on 2026-09-22. Commit `a12b098` of 2026-09-23 recorded
      that the maintainer made the choice.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/stream-process-runs.md?plain=1#L328-L342>
  - Streaming types keep a metadata-only `Debug`.
    - Home: `docs/process-diagnostics.md`, section "Automatic Diagnostics".
      Add it after "Returned event bytes remain exact." at line 36.
    - Text: "Streaming requests, commands, and events keep a metadata-only
      `Debug`. The removal of `Debug` was rejected, because tests and callers
      need safe structural diagnostics."
    - Record: `plans/stream-process-runs.md`, lines 352-377. Commit `3c9e2f6`
      recorded both findings, and commit `3853587` applied the choice, both
      on 2026-09-23. The maintainer made the choice.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/stream-process-runs.md?plain=1#L352-L377>
  - Explicit modes for test roots instead of a umask, in a crate README.
    - Home: `crates/sandbox-e2b/README.md`, section "Development". Add it
      after the paragraph about `private_tempdir` at lines 322-326.
    - Text: "Do not set a umask in `cargo xtask check` instead. Do not
      document a umask requirement either. Explicit modes keep plain
      `cargo test` passing and keep the helper safety checks unchanged."
    - Record: `plans/stream-process-cwd-env-and-write-redaction.md`, lines
      262-271. The user selected this choice. Commit `b9d5367` recorded it
      on 2026-10-03.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/stream-process-cwd-env-and-write-redaction.md?plain=1#L262-L271>
  - Plain strings that the adapter validates.
    - Home: `docs/e2b-adapter.md`, section "Transport And Reconciliation".
      Add it after "an unusable inventory row is retryable provider
      unavailability." at lines 146-147.
    - Text: "Provider IDs, snapshot names, and credentials stay plain strings
      in the public API of the E2B control client. The adapter validates them
      at the boundary. Validated wrapper types were rejected, because they
      change that API."
    - Record: `plans/extract-sandbox-interface-and-e2b.md`, lines 769-777,
      794-802, 851-879, and 913-923, in commits `dcebe8a`, `e70481a`, and
      `44ff302` of 2026-09-22. Each choice kept the public API unchanged.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/extract-sandbox-interface-and-e2b.md?plain=1#L769-L923>
  - The control client stays public.
    - Home: `docs/e2b-adapter.md`, section "Configuration". Add it after
      "before building the authenticated transport." at lines 26-27.
    - Text: "The control client stays public. A private constructor was
      rejected. The public constructor repeats the checks instead, so that
      every construction path is safe."
    - Record: `plans/extract-sandbox-interface-and-e2b.md`, lines 760-768.
      Commit `dcebe8a` recorded it on 2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/extract-sandbox-interface-and-e2b.md?plain=1#L760-L768>
  - Cleanup completes the handoff of a recovered write.
    - Home: `docs/sandbox-contract.md`, section "Bounds And Failure Safety".
      Add it after "Without that usable workload identity, the outcome is
      unconfirmed." at lines 150-151.
    - Text: "Cleanup completes this handoff instead of always returning
      unconfirmed. Always returning unconfirmed was rejected, because it
      discards a provable successful write."
    - Record: `plans/extract-sandbox-interface-and-e2b.md`, lines 1014-1027.
      Commit `5d3c2ad` recorded it on 2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/extract-sandbox-interface-and-e2b.md?plain=1#L1014-L1027>
  - The backend reconnects the snapshot source itself.
    - Home: `docs/sandbox-contract.md`, the list under "Image construction is
      a caller-persisted state machine". Add it to step 4, at lines 123-127.
    - Text: "The backend reconnects the source itself after exactly one
      completed candidate. A separate resume phase that the caller owns was
      rejected, because it changes the API."
    - Record: `plans/extract-sandbox-interface-and-e2b.md`, lines 1081-1089.
      Commit `2e0d433` recorded it on 2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/extract-sandbox-interface-and-e2b.md?plain=1#L1081-L1089>
  - One 256 MiB bound for terminal transcripts.
    - Home: `docs/sandbox-contract.md`, section "Bounds And Failure Safety".
      Add it after "cannot set its provider transcript limit above the
      shared 256 MiB regular-file ceiling." at lines 211-212.
    - Text: "A separate reader for larger transcripts was rejected. Recording
      and reading share one 256 MiB bound, so the library needs no second
      file protocol."
    - Record: `plans/extract-sandbox-interface-and-e2b.md`, lines 1109-1118.
      Commit `2e0d433` recorded it on 2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/extract-sandbox-interface-and-e2b.md?plain=1#L1109-L1118>
- **Problem:** The docs miss sixteen decisions in force, in full or in part.
  A change can break the two requirements and still match the docs. An agent
  can propose one of the fourteen rejected alternatives again.
- **Impact if no action:** A review cannot catch a change that breaks either
  requirement, because no doc states the missing parts. Agents can also
  propose an alternative that a maintainer already turned down.
- **Root question:** Where does each decision in force live after its plan is
  deleted, so that agents keep it and do not propose its rejected
  alternative again?
- **Options:**
  - **A.** Add each entry above to its home with its text. Effort: Large.
    - Split `docs/sandbox-contract.md`, with 428 lines, and
      `docs/e2b-adapter.md`, with 471 lines, into linked pages of about 250
      lines or less, as the `documentation` skill requires.
    - Two sections pass that target alone, with 252 and 255 lines, so each
      needs its own split:
      - "Bounds And Failure Safety", `docs/sandbox-contract.md:136-387`.
      - "Files, Processes, Terminals, And Screens", `docs/e2b-adapter.md:197-451`.
    - The same skill requires a second move in each protocol doc that the
      change touches. The change moves out function names, private type
      names, internal call order, and descriptions of mechanics. Entry points
      go to the crate README under `### Key Code`. Mechanics go to doc
      comments. A review of 2026-11-04 found these cases:
      - `docs/sandbox-contract.md:221` names `SandboxBackend::stream_process`,
        and line 229 names `validate_execution_context()`.
      - `docs/sandbox-contract.md:228-230` gives an internal call order.
      - `docs/sandbox-contract.md:243-261` describes the mechanics of the
        provider reader, its staging, and its batches.
      - `docs/e2b-adapter.md:24` names `ReqwestE2bControlApi::new`, and line
        316 names `ConnectProcessTransport::stream_process`.
      - `docs/process-diagnostics.md:33` names the private type
        `StreamSettings`.
    - The moved mechanics change Rust doc comments, so the change is not
      docs-only. Run the full `cargo xtask check` gate.
    - The split moves lines. Update the line citations of the three docs in
      the other items under `plans/follow-ups/`. Update the doc names at
      `AGENTS.md:9-10`, `:98-99`, and `:102` when a page gets a new name.
      Find each link with
      `grep -rn "sandbox-contract.md\|e2b-adapter.md\|process-diagnostics.md" --include=*.md .`.
  - **B.** Add only the two requirements, and leave the fourteen rejected
    alternatives at the permalinks. The split and the code-name move then
    apply only to `docs/sandbox-contract.md`. Agents can still propose the
    rejected alternatives. Effort: Medium.
- **Left out:**
  - Only two approaches are real, because the `documentation` skill fixes the
    home of a decision in force. The options differ in how many entries
    move.
  - Change nothing. The two requirements and every reason then stay only at
    the permalinks, so the goal fails.
  - Write one separate file that lists all decisions. The `documentation`
    skill forbids a dated decision log in a protocol doc.
  - Write the decisions as code comments. The code is the home of mechanics,
    not of decisions in force.
- **Wider fix:** None. The `plans` skill already requires that the protocol
  docs hold each decision in force before a plan is deleted. This item is
  the deferred part of that step.
- **Previous examples:** Commit `7d6e703` of 2026-09-22 added a stream
  outcome rule to `docs/sandbox-contract.md`. Commit `f010faf` replaced it the
  same day. The merge commit `fa11e0e` of 2026-09-23 dropped the `f010faf`
  version.
- **Related:**
  - `plans/follow-ups/docs-hold-history-and-provider-rule.md`: both change
    `docs/sandbox-contract.md`. This item owns the split and the move of code
    names, so do both in one branch.
  - `plans/follow-ups/file-read-results-debug-prints-bytes.md`: both change
    `docs/sandbox-contract.md`. The first change that lands splits it.
  - `plans/follow-ups/control-client-extends-one-shot-deadline.md`: both
    change `docs/e2b-adapter.md`. The first change that lands splits it.
- **Recommendation:** A, because only the deleted plans hold these reasons
  now, and the `documentation` skill puts each decision in force next to its
  requirement.
- **Action:** fix after merge, apply A, because a design call of 2026-11-04
  kept the doc splits out of the branch that deleted the plans.
