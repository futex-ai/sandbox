# Implementation Review Contract

The repository's agent instructions apply this contract to native base-branch
reviews. The Codex CLI does not currently accept supplemental prompt text
together with its native `--base` target.

Review the complete current-branch diff against `origin/main`.

Do not edit files, run formatters, or make commits. Report only concrete,
actionable defects introduced by this branch. Ignore style preferences unless
they create a correctness, security, compatibility, test, or maintenance risk.

Explain every finding in simple, direct language and assume the reader has no
prior context about this repository, feature, implementation, or its technical
concepts. For each finding, include:

1. A numbered title and severity (`critical`, `high`, `medium`, or `low`).
2. The file and exact line where the problem appears.
3. The context needed to understand what the code is meant to do.
4. The impact if nothing changes.
5. Lettered solution options (`A`, `B`, and so on), including meaningful
   tradeoffs when more than one safe fix exists.
6. One clearly recommended option and why it is preferred.

Check especially for missing copied behavior, provider calls in default tests,
broken recovery or identity fencing, secret exposure, unsafe file/process
handling, unvalidated deployment conventions, stale source-only paths,
unintended dependencies, and documentation that disagrees with the code.

If there are no actionable findings, say so plainly.
