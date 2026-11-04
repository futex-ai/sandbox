# Docs miss sixteen decisions in force

- **Severity:** Medium. **Effort:** Medium. **Confidence:** High. **Harder later:** No.
- **Kind:** Fix after merge.
- **Category:** Docs or spec.
- **Added:** 2026-11-04, from the branch `conductor/agents-md-sandbox`. That
  branch deleted the eight completed plans that recorded these decisions.
- **Location:** `docs/sandbox-contract.md` at lines 123-127, 150-151,
  188-191, 211-212, 234, 243-247, 259-261, 335-338, 394-397, and 409-410.
  `docs/e2b-adapter.md` at lines 24-27 and 145-147.
  `docs/process-diagnostics.md:34-36`. `crates/sandbox-e2b/README.md:326-330`.
- **Reason:** A design call of 2026-11-04 kept doc splits out of the branch
  that deleted the plans, and this fix needs two splits.

- **Terms:**
  - **Decision in force:** a recorded choice that still applies. The
    `documentation` skill from `futex-ai/skills` puts it in the protocol doc,
    next to its requirement.
  - **Rejected alternative:** an option that a recorded decision turned down,
    with the reason.
- **Background:**
  - The `documentation` skill says that a decision in force gives "the
    choice, the rejected alternative, and the reason, in one or two
    sentences". Its purpose is "to stop an agent from proposing the rejected
    alternative again".
  - On 2026-11-04 the branch `conductor/agents-md-sandbox` deleted
    `plans/README.md` and eight completed plans. Their work had merged. Each
    plan was the dated record of its decisions. The permalinks below keep
    each record at commit `6f4b0e540703ca5657a3bd28dd917a4263cdd0c0`.
  - An analysis of the eight plans found that the docs hold almost every
    decision in force. The sixteen entries below are the exceptions. Two are
    requirements. Fourteen are rejected alternatives that the docs neither
    state nor rule out.
  - The code already has each behaviour below, and a recorded decision chose
    each one. So no entry changes behaviour, and no entry needs a new user
    decision.
  - One more candidate is already held. The rule that an adapter rejects a
    policy variant that it cannot apply is at `docs/sandbox-contract.md:88-92`.
    Do not add it again.
  - `docs/sandbox-contract.md` has 428 lines, and `docs/e2b-adapter.md` has
    471 lines. The `documentation` skill says: "Keep each doc short, about
    250 lines. When you touch a doc that passes the target, split it into
    linked pages in the same change."
  - This command finds each link to these docs:
    `grep -rn "sandbox-contract.md\|e2b-adapter.md\|process-diagnostics.md" --include=*.md .`.
    Today every link points to a whole file, not to a section.
  - The line numbers below are the lines when this item was added. Each entry
    also quotes the sentence before the new text. Then the place stays easy
    to find after an edit or a split.
  - The streaming outcome comes before the kill wait. It is a requirement.
    - Home: `docs/sandbox-contract.md`, section "Bounds And Failure Safety",
      the streaming paragraph. Add it after the sentence at lines 246-247.
      That sentence ends "when a PID was decoded and no process end was
      decoded."
    - Text: "A stream that its consumer still holds receives its terminal
      outcome and EOF before the backend waits for this kill. A stalled kill
      cannot delay the outcome past the absolute deadline. A cleanup grace
      period after the deadline was rejected, because it would make the
      absolute deadline meaningless."
    - Record: `plans/stream-process-runs.md`, lines 93-101. Commit `99ae681`
      recorded the finding for the maintainer. Commit `7d6e703` applied the
      choice. Both are from 2026-09-22.
    - Commit `7d6e703` added this sentence to the contract:

      > For an owned stream, the backend sends its terminal outcome and
      > closes the stream before awaiting cleanup, so the cleanup allowance
      > cannot extend the observable absolute deadline.

      The merge commit `fa11e0e` of 2026-09-23 dropped it.
    - `crates/sandbox-e2b/src/process/stream_run.rs:111-119` still sends the
      outcome before the kill. The test
      `deadline_outcome_and_eof_precede_stalled_kill_cleanup` at
      `crates/sandbox-e2b/src/process/_tests_/stream_run/cleanup_tests.rs:86`
      checks it. `docs/e2b-adapter.md:285-289` only implies the rule.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/stream-process-runs.md?plain=1#L93-L101>
  - Any HTTP response from the other host fails the network probe. It is a
    requirement.
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
    - `crates/sandbox-interface/src/conformance_network.rs:13` sends the
      second fetch without `--fail`. Lines 46-55 fail the probe when curl
      exits with 0. The test
      `disallowed_fetch_does_not_turn_http_errors_into_denial_success` is at
      `crates/sandbox-interface/src/_tests_/conformance_network_tests.rs:6`.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/egress-allowlist-policy.md?plain=1#L63-L84>
  - No tombstone process and no create acknowledgment. It is a rejected
    alternative.
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
  - Close keeps the identity record instead of draining the transcript. It
    is a rejected alternative.
    - Home: the same paragraph, after the same sentence.
    - Text: "Close does not drain or persist the transcript before it
      returns. Later cursor reads return the final bytes through the kept
      identity record. A drain at close was rejected, because it adds an
      ingestion path beside the cursor-based read API."
    - Record: `plans/harden-sandbox-and-terminal-create-recovery.md`, lines
      303-314. Commit `b832b25` recorded and applied it on 2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/harden-sandbox-and-terminal-create-recovery.md?plain=1#L303-L314>
  - A returned routing domain is ignored, not compared. It is a rejected
    alternative.
    - Home: `docs/sandbox-contract.md`, section "Bounds And Failure Safety".
      Add it after "even when a control transport is injected." at lines
      190-191.
    - Text: "A returned domain that differs from configuration does not fail
      the call. The adapter ignores it, because configuration is the trusted
      routing authority."
    - Record: `plans/harden-stream-completion-and-credential-routing.md`,
      lines 34-44. Commit `b59399f` recorded and applied it on 2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/harden-stream-completion-and-credential-routing.md?plain=1#L34-L44>
  - One probe process also checks stderr. It is a rejected alternative.
    - Home: `docs/sandbox-contract.md`, section "Conformance". Add it after
      "so conforming images need a standard shell but no harness-only
      executable." at lines 396-397.
    - Text: "One process checks the working directory, the environment,
      stdout, and stderr. A second probe for stderr only was rejected,
      because one provider process covers all four."
    - Record: `plans/process-run-cwd-and-env.md`, lines 78-86. The user
      selected this choice. Commit `3b75f1b` recorded it on 2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/process-run-cwd-and-env.md?plain=1#L78-L86>
  - The exit event keeps a flag instead of a separate signal event. It is a
    rejected alternative.
    - Home: `docs/sandbox-contract.md`, the streaming paragraph. Add it after
      "Signal termination reports `exited: false`" at line 234.
    - Text: "A separate signal-termination event was rejected. The `exited`
      flag keeps the status that collected runs already report."
    - Record: `plans/stream-process-runs.md`, lines 84-92. Commit `99ae681`
      recorded it, and commit `7d6e703` applied the choice, both on
      2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/stream-process-runs.md?plain=1#L84-L92>
  - Cleanup keeps queued data. It is a rejected alternative.
    - Home: `docs/sandbox-contract.md`, the streaming paragraph. Add it after
      "cleanup does not wait for consumer capacity." at lines 260-261.
    - Text: "Cleanup does not discard data that is already in the consumer
      queue. A consumer-lag outcome that drops that data was rejected,
      because a consumer that resumes polling must receive it in order."
    - Record: `plans/stream-process-runs.md`, lines 152-167. Commit `417cb5a`
      recorded it, and commit `f010faf` applied the choice, both on
      2026-09-22. The maintainer made the choice.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/stream-process-runs.md?plain=1#L152-L167>
  - A full consumer queue does not end the stream. It is a rejected
    alternative.
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
  - Streaming types keep a metadata-only `Debug`. It is a rejected
    alternative.
    - Home: `docs/process-diagnostics.md`, section "Automatic Diagnostics".
      Add it after "Returned event bytes remain exact." at line 36. This doc
      has 87 lines, so this entry needs no split.
    - Text: "Streaming requests, commands, and events keep a metadata-only
      `Debug`. The removal of `Debug` was rejected, because tests and callers
      need safe structural diagnostics."
    - Record: `plans/stream-process-runs.md`, lines 352-377. Commit `3c9e2f6`
      recorded both findings, and commit `3853587` applied the choice, both
      on 2026-09-23. The maintainer made the choice.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/stream-process-runs.md?plain=1#L352-L377>
  - Explicit modes for test roots instead of a umask. It is a rejected
    alternative, and its home is a crate README.
    - Home: `crates/sandbox-e2b/README.md`, section "Development". Add it
      after the paragraph about `private_tempdir` at lines 326-330.
    - Text: "Do not set a umask in `cargo xtask check` instead. Do not
      document a umask requirement either. Explicit modes keep plain
      `cargo test` passing and keep the helper safety checks unchanged."
    - Record: `plans/stream-process-cwd-env-and-write-redaction.md`, lines
      262-271. The user selected this choice. Commit `b9d5367` recorded it
      on 2026-10-03.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/stream-process-cwd-env-and-write-redaction.md?plain=1#L262-L271>
  - Plain strings that the adapter validates. It is a rejected alternative.
    - Home: `docs/e2b-adapter.md`, section "Transport And Reconciliation".
      Add it after "an unusable inventory row is retryable provider
      unavailability." at lines 146-147.
    - Text: "Provider IDs, snapshot names, and credentials stay plain strings
      in the public API. The adapter validates them at the boundary.
      Validated wrapper types were rejected, because they change the public
      API."
    - Record: `plans/extract-sandbox-interface-and-e2b.md`, lines 769-777,
      794-802, 851-879, and 913-923. Commits `dcebe8a`, `e70481a`, and
      `44ff302` recorded them on 2026-09-22. Each choice kept the public API
      unchanged.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/extract-sandbox-interface-and-e2b.md?plain=1#L769-L923>
  - The control client stays public. It is a rejected alternative.
    - Home: `docs/e2b-adapter.md`, section "Configuration". Add it after
      "before building the authenticated transport." at lines 26-27.
    - Text: "The control client stays public. A private constructor was
      rejected. The public constructor repeats the checks instead, so that
      every construction path is safe."
    - Record: `plans/extract-sandbox-interface-and-e2b.md`, lines 760-768.
      Commit `dcebe8a` recorded it on 2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/extract-sandbox-interface-and-e2b.md?plain=1#L760-L768>
  - Cleanup completes the handoff of a recovered write. It is a rejected
    alternative.
    - Home: `docs/sandbox-contract.md`, section "Bounds And Failure Safety".
      Add it after "Without that usable workload identity, the outcome is
      unconfirmed." at lines 150-151.
    - Text: "Cleanup completes this handoff instead of always returning
      unconfirmed. Always returning unconfirmed was rejected, because it
      discards a provable successful write."
    - Record: `plans/extract-sandbox-interface-and-e2b.md`, lines 1014-1027.
      Commit `5d3c2ad` recorded it on 2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/extract-sandbox-interface-and-e2b.md?plain=1#L1014-L1027>
  - The backend reconnects the snapshot source itself. It is a rejected
    alternative.
    - Home: `docs/sandbox-contract.md`, the list under "Image construction is
      a caller-persisted state machine". Add it to step 4, at lines 123-127.
    - Text: "The backend reconnects the source itself after exactly one
      completed candidate. A separate resume phase that the caller owns was
      rejected, because it changes the API."
    - Record: `plans/extract-sandbox-interface-and-e2b.md`, lines 1081-1089.
      Commit `2e0d433` recorded it on 2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/extract-sandbox-interface-and-e2b.md?plain=1#L1081-L1089>
  - One 256 MiB bound for terminal transcripts. It is a rejected
    alternative.
    - Home: `docs/sandbox-contract.md`, section "Bounds And Failure Safety".
      Add it after "cannot set its provider transcript limit above the
      shared 256 MiB regular-file ceiling." at lines 211-212.
    - Text: "A separate reader for larger transcripts was rejected. Recording
      and reading share one 256 MiB bound, so the library needs no second
      file protocol."
    - Record: `plans/extract-sandbox-interface-and-e2b.md`, lines 1109-1118.
      Commit `2e0d433` recorded it on 2026-09-22.
    - <https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/extract-sandbox-interface-and-e2b.md?plain=1#L1109-L1118>
- **Problem:** Sixteen decisions in force are only in the deleted plans. A
  change can break the two requirements and still match the docs. An agent
  can propose one of the fourteen rejected alternatives again.
- **Impact if no action:** A review cannot catch a change that breaks the
  streaming outcome rule or the network probe rule, because no doc states
  them. Agents can spend review rounds on alternatives that a maintainer
  already turned down.
- **Root question:** Where does each decision in force live after its plan is
  deleted, so that agents keep it and do not propose its rejected
  alternative again?
- **Options:**
  - **A.** Add each entry above to its home with its text. Split
    `docs/sandbox-contract.md` and `docs/e2b-adapter.md` into linked pages of
    about 250 lines or less. Update each link to a moved section. Effort:
    Medium.
  - **B.** Add only the two requirements, and leave the fourteen rejected
    alternatives at the permalinks. The split of `docs/sandbox-contract.md`
    still applies. Agents can still propose the rejected alternatives.
    Effort: Medium.
- **Left out:**
  - Only two approaches are real, because the `documentation` skill fixes the
    home of a decision in force. The options differ in how many entries
    move.
  - Change nothing. The two requirements and every reason then stay only at
    the permalinks, so the goal fails.
  - Write one separate file that lists all decisions. The `documentation`
    skill forbids a dated decision log in a protocol doc, and it keeps each
    decision next to its requirement.
  - Write the decisions as code comments. The code is the home of mechanics,
    not of decisions in force.
- **Wider fix:** None. The `plans` skill already requires that the protocol
  docs hold each decision in force before a plan is deleted. This item is
  the deferred part of that step.
- **Previous examples:** The merge commit `fa11e0e` of 2026-09-23 dropped the
  streaming outcome rule from `docs/sandbox-contract.md`. Commit `7d6e703`
  of 2026-09-22 had added it.
- **Related:**
  - `plans/follow-ups/docs-hold-history-and-provider-rule.md`: both change
    `docs/sandbox-contract.md`, so do both in one branch and split the doc
    once.
  - `plans/follow-ups/file-read-results-debug-prints-bytes.md`: both change
    `docs/sandbox-contract.md`. The first change that lands splits it.
  - `plans/follow-ups/control-client-extends-one-shot-deadline.md`: both
    change `docs/e2b-adapter.md`. The first change that lands splits it.
- **Recommendation:** A, because only the deleted plans hold these reasons
  now, and the `documentation` skill puts each decision in force next to its
  requirement.
- **Action:** fix after merge, apply A, because a design call of 2026-11-04
  kept the doc splits out of the branch that deleted the plans.
