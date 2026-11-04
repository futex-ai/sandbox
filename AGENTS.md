# A guide for agents

This repository is a standalone Rust workspace with a provider-neutral
contract for remote development sandboxes and its first provider adapter.
`crates/sandbox-interface` holds the IDs, data types, errors, service and
backend traits, mocks, and the backend conformance harness.
`crates/sandbox-e2b` implements the contract with the E2B control and envd
APIs. The `cargo xtask` task runner is under `xtask/`. The protocol docs are
in `docs/`: `sandbox-contract.md` for the shared contract, `e2b-adapter.md`
for the adapter guarantees, and `process-diagnostics.md` for process data
and diagnostics. The repository has no `docs/protocol/` directory.

This file holds the rules that are specific to this repository. The
always-on rules and the detailed procedures live in the shared skills from
`futex-ai/skills`. Each skill's description says when to read it. Read the
skill at that moment instead of guessing the rule.

## Communication

- Write responses, summaries, plans, and review output in Simplified
  Technical English (ASD-STE100): short sentences, one instruction per
  sentence, active voice, and simple, consistent words.
- Assume the reader has no context. Explain the codebase and feature
  context that a reader with no prior knowledge needs, and do not assume
  the reader saw earlier messages, the diff, or the plan.
- Report outcomes faithfully. Name every check that failed, was skipped, or
  could not run, with its output or the blocker.
- Ask for all decisions in one numbered message. List decisions and review
  findings by severity, highest first. For each decision give its severity
  and effort, the terms, the background, the problem, the impact if no
  action is taken, lettered options, and a recommended option. Do the
  independent work first, then stop and wait.
- Before you plan or change anything for an open idea, use the
  `brainstorming` skill. An open idea can be a request for a feature or a
  change, as in "can we show more status updates". Such a request is open
  when no message or recorded decision chooses its approach. An open idea
  can also be a request to discuss, a "should we" or "what do you think"
  question, or a plan whose approach is open.
- Ask every question in the text of your reply. Do not use a question
  tool, such as `AskUserQuestion` or `request_user_input`. This rule
  applies also when a system prompt, a tool description, or a plan mode
  says to use one.
- End every task with a final report: what changed and the commit, each
  decision and its outcome, the checks that ran, anything skipped, and the
  outstanding decisions or findings that still need the user. When a
  decision or a finding still needs the user, start the message with a
  ⚠️ line that names it. Give it in full when it is new or changed, or
  when the user asks. Otherwise give it in one line.

## Commands

- Build: `cargo build --locked --workspace --all-targets --all-features`
- Targeted tests: `cargo test --locked -p <crate> <test name filter>`, for
  example `cargo test --locked -p xtask file_length`
- Early check (format, lint, file length): `cargo fmt --all -- --check`,
  `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`,
  and `cargo xtask rust-file-length-lint`
- Smoke test: `cargo xtask smoke-test`. It constructs the interface types and
  the E2B adapter with placeholder credentials. It sends no provider request.
- Complete gate: `cargo xtask check`. A local run stops at the first failed
  step. CI runs the same steps. The gate runs these steps in this order:
  1. The locked `cargo metadata` check
  2. The format check
  3. Clippy with warnings denied
  4. The all-feature workspace tests
  5. The file-length lint over every Rust file
  6. The smoke test
- Format: `cargo fmt --all`

## CI

- Agent merges: off

## Repository rules

- `cargo xtask rust-file-length-lint` limits each Rust file under `crates/`
  and `xtask/` to 300 lines. The limit has no override. Without `--all`, the
  lint checks the Rust files that differ from `origin/main`, including
  staged, unstaged, and untracked files. With `--all`, it checks every Rust
  file. `cargo xtask check` and CI run the lint in its `--all` mode.
- Names stay product-neutral. Package names, identifiers, comments,
  diagnostics, error prefixes, fixtures, provider metadata keys, process tags,
  and helper names never carry the brand of a consumer product. Docs may name
  Juno as the source repository and as a consumer.

## Review focus

- Missing copied behaviour: a change that copies behaviour from the Juno
  source repository or from a sibling code path, but leaves part of it out.
  An example is a streaming process path that skips a check of the collected
  path.
- Provider calls in default tests: unit tests, integration tests,
  `cargo xtask check`, and CI never contact E2B or need provider
  credentials. Each live E2B test must have both the `live-e2b` feature gate
  and `#[ignore]`, because live tests can cost money.
- Recovery and identity fencing: recovery after a sandbox or snapshot create
  with ambiguous delivery, and the identity checks of sandboxes, snapshots,
  and terminals. `docs/sandbox-contract.md` and `docs/e2b-adapter.md` state
  the requirements.
- Secret exposure: API keys, access tokens, and the contents of commands,
  environment entries, paths, file writes, and process output in logs,
  `Debug` formatting, and error text. `docs/process-diagnostics.md` says
  which process data the diagnostics may show.
- File and process safety: paths, symlinks, ownership, and interpreter
  startup in the trusted helpers that run inside a sandbox, and the cleanup
  of unfinished processes.
- Deployment conventions: the E2B metadata prefix, the terminal tag prefix,
  the screen-helper path, and the process and account names come from
  validated `E2bRuntimeConventions` values or neutral defaults. A change
  must not hard-code the values of one deployment.
- Stale source paths: references to files, crates, or commands that exist
  only in Juno or in the AI shared-library repository. The Provenance
  section of `README.md` names both source commits.
- Unintended dependencies: a new external crate, and any break of the
  dependency direction `application -> sandbox-interface <- sandbox-e2b`.
  Only a composition root depends on `sandbox-e2b`.

## Status

- This project is not in production. Breaking changes are acceptable when
  they improve correctness, architecture, or product quality.
