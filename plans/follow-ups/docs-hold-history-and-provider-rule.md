# Docs hold history and a misplaced provider rule

- **Severity:** Low. **Effort:** Small. **Confidence:** High. **Harder later:** No.
- **Kind:** Fix after merge.
- **Category:** Docs or spec.
- **Added:** 2026-11-04, from the branch `conductor/agents-md-sandbox`. The
  analysis of the deleted plans found it.
- **Location:** `docs/process-diagnostics.md:71-73`,
  `docs/process-diagnostics.md:75`, `crates/sandbox-interface/README.md:100-102`,
  `docs/sandbox-contract.md:197-198`.
- **Reason:** The fix deletes a sentence from `docs/sandbox-contract.md`,
  which must be split when a change touches it.

- **Terms:**
  - **Protocol doc:** a doc that holds the requirements and the decisions in
    force of one boundary. Here they are `docs/sandbox-contract.md`,
    `docs/e2b-adapter.md`, and `docs/process-diagnostics.md`.
- **Background:**
  - The `documentation` skill from `futex-ai/skills` says that a protocol
    doc must not "record history, such as what the code did before a
    change". The record of the change holds history.
  - History in `docs/process-diagnostics.md`. Lines 71-73 say: "The former
    `output` field and `from_captured_output` normalization/redaction API are
    removed. There is no secret matching, ANSI stripping, or replacement
    chain in failure mapping." Line 75 says: "An image error no longer
    includes a command-output snippet."
  - The current rules already hold each fact that these sentences need.
    `docs/process-diagnostics.md:3-5` says that the library does not discover
    or replace secrets. Lines 62-63 list the four fields of
    `ImageCommandFailure`. Lines 70-71 say: "Captured text is never copied
    into handled errors, `Debug`, or serialized failure metadata."
  - History in `crates/sandbox-interface/README.md:100-102`. The passage says
    that `ImageCommandFailure` holds only exit status, the retained output
    byte count, and capture truncation. Then it adds: "the former output
    snippet and normalization API are removed". The table of homes in the
    same skill puts history in the plan of the change and then in its pull
    request description. A README is not its home.
  - Commit `ce77662` of 2026-09-22 wrote these history sentences when it
    removed the output snippet.
  - A provider rule in the neutral contract. `docs/sandbox-contract.md:197-198`
    says: "For E2B, a sandbox ID must fit the lowercase DNS label used for
    envd." Lines 3-5 say that the contract describes the boundary between a
    trusted consumer and "any concrete sandbox provider".
  - `docs/e2b-adapter.md:142-145` already holds the E2B rule with its detail.
    A returned sandbox ID is a lowercase DNS-label fragment of at most 57
    bytes. Then the port prefix and the ID fit E2B's 63-byte envd label.
  - The general rule stays in the contract at lines 196-197: creation and
    inventory "must also reject returned identities that their later
    provider routes cannot use". Commit `c007311` of 2026-09-22 added the E2B
    sentence.
  - The `documentation` skill also says that a change moves code names and
    mechanics out of each protocol doc that it touches. For example,
    `docs/process-diagnostics.md:32-33` names the private type
    `StreamSettings`.
  - `docs/sandbox-contract.md` has 428 lines. The same skill says: "When you
    touch a doc that passes the target, split it into linked pages in the
    same change." The target is about 250 lines.
- **Problem:** Three passages describe code that no longer exists, with the
  words "former", "removed", and "no longer". One sentence of the
  provider-neutral contract states an E2B rule that the adapter doc already
  holds.
- **Impact if no action:** Readers spend time on an API that no longer
  exists. The author of a second provider can read the E2B limit as a rule
  for every provider. The cost is small.
- **Root question:** Where does each passage live, so that each doc states
  only the current rules of its own boundary?
- **Options:**
  - **A.** Delete the three history passages, because the current rules
    already state their facts. Delete the E2B sentence at
    `docs/sandbox-contract.md:197-198`, and keep the general rule at lines
    196-197. Move each code name out of each protocol doc that the change
    touches. Do this in the branch of
    `plans/follow-ups/docs-miss-decisions-in-force.md`, which splits the
    contract. Alone, the split makes this change Medium. Effort: Small.
- **Left out:**
  - Only one approach is real, because the `documentation` skill names the
    home of history and of provider rules.
  - Rewrite the history as new rules. The current rules already state each
    fact, so new text would repeat them.
  - Keep the E2B sentence as an example. The `documentation` skill says:
    "Do not list every case: state the rule once and let the tests hold the
    cases."
  - Change nothing. The docs keep content that the `documentation` skill
    puts elsewhere.
- **Wider fix:** None. The `documentation` skill is the wider fix for the
  content of docs, and it is in force.
- **Previous examples:** Commit `ce77662` of 2026-09-22 wrote the history
  passages. Commit `c007311` of 2026-09-22 added the E2B sentence to the
  contract.
- **Related:** `plans/follow-ups/docs-miss-decisions-in-force.md`: both change
  `docs/sandbox-contract.md`, and that item splits it. Do both in one branch.
- **Recommendation:** A, because the current rules already hold each fact,
  and the adapter doc already holds the E2B rule.
- **Action:** fix after merge, apply A, because the fix touches
  `docs/sandbox-contract.md`, and the branch that splits that doc is the
  cheapest place for it.
