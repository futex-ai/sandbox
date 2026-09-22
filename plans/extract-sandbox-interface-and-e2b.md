# Extract Sandbox Interface And E2B Provider From Juno

## Summary

Create a standalone Rust workspace for the provider-neutral sandbox contract
and its first concrete provider by extracting Juno's existing sandbox
interface and E2B adapter from the clean `main` revision
`64e6dd1c1fcc0d44c73fa1b31879cb4ba80448c6` into:

- `crates/sandbox-interface`
- `crates/sandbox-e2b`

Use `/Users/calummoore/projects/futex/ai` at revision
`35da3dd9f5b7316bcfbdfa99c665e406220ffaec` as the standalone shared-library
reference for workspace metadata, Rust tooling, CI, documentation, plans, and
review automation. Preserve the source behavior and `0.2.0` version while
renaming packages, Rust import paths, documentation, diagnostics, and runtime
conventions to make the resulting library fully product-neutral.

The resulting dependency direction is:

```text
consumer -> sandbox-interface <- sandbox-e2b
```

Future providers should implement the interface in sibling adapter crates;
consumers should not depend on E2B unless they are composition roots.

## Source And Destination Map

| Source | Destination | Treatment |
| --- | --- | --- |
| Juno sandbox interface source tree | `crates/sandbox-interface` | Copy all source, tests, and crate documentation, then apply neutral naming. |
| Juno E2B adapter source tree | `crates/sandbox-e2b` | Copy all source, unit tests, integration tests, and live-test modules, then apply neutral naming. |
| `ai/AGENTS.md` | `AGENTS.md` | Copied with this plan as the shared-library agent guidance. |
| `ai` workspace scaffolding | Repository root, `xtask/`, and `.github/` | Adapt for the two sandbox crates; do not copy AI-specific behavior. |

The source inventory contains 34 interface files and 79 E2B files. Every Rust
file currently fits the copied 300-line limit. Record the source revision again
at implementation time and stop if the Juno worktree is dirty or its `main`
revision differs; update this plan's provenance before copying a newer source.

## Naming And Compatibility

The standalone repository must contain no legacy product branding in package
names, Rust identifiers, documentation, comments, diagnostics, fixtures,
provider metadata conventions, process tags, or helper executable names.

- Name the packages `sandbox-interface` and `sandbox-e2b`, with Rust import
  paths `sandbox_interface` and `sandbox_e2b`.
- Describe caller-owned identifiers as platform, consumer, or sandbox handles
  rather than tying them to one product.
- Use neutral error prefixes and test fixture names.
- Replace deployment-specific E2B metadata keys, terminal tag prefixes, and
  screen-helper paths with validated adapter configuration or neutral defaults.
- Keep compatibility with pre-existing deployment resources outside this
  library: the later Juno integration must explicitly supply or migrate its
  deployment conventions rather than restoring branded constants here.
- Require a case-insensitive, repository-wide legacy-brand audit to return no
  matches before the extraction is complete.

## Scope Boundaries

This extraction deliberately does not copy:

- Juno's higher-level sandbox lifecycle orchestration crate, which depends on
  application persistence interfaces rather than defining the provider boundary.
- `infra/e2b`, whose compatibility fixtures belong with the separately owned
  E2B template release project.
- Juno server, worker, preview-router, environment, browser, deployment, or
  store crates that consume the sandbox interface.
- Juno application protocol pages whose relative link graph depends on the
  monorepo. Extract only the standalone contract documentation needed here.
- The Juno dependency cutover. That requires a follow-up Juno change after this
  repository has a reviewed, pushed commit that Juno can pin by Git revision.

The shared `internal-error` crate remains an external Git dependency pinned to
the same revision used by both source crates:
`6477922a1c4db8c3e35189af69f0147464004b99`.

Default checks and CI must never call E2B or require provider credentials. The
existing `live-e2b` feature and ignored live tests remain opt-in and billable.

## Review Implementation

`cargo xtask review` will be a manual, post-push wrapper around the installed
Codex CLI's base-branch review mode. It will not implement its own source-code
analysis or run in CI.

- Add `docs/implementation-review-prompt.md` with the repository-specific
  review contract and reference it from `AGENTS.md`: inspect the complete
  branch diff, do not edit files, report only actionable findings, and give
  every finding a severity, file and line, context, impact, lettered solution
  options, and recommended option.
- Implement `xtask/src/review.rs` against the shared injected command-runner
  boundary used by the other automation commands.
- Fetch `origin/main`, require a clean worktree with no untracked files, require
  the current branch to have an upstream, and require local `HEAD` to equal the
  upstream revision. These checks enforce the required commit-then-push order.
- Use Codex's native base-branch review command from the workspace root. Native
  target flags and supplemental prompt input are mutually exclusive in the
  installed CLI, so repository instructions carry the reporting contract:

  ```sh
  codex exec review --base origin/main --ephemeral
  ```

- Do not bypass Codex's Git-repository check. Inherit stdout and stderr so the
  findings remain visible, translate spawn and nonzero-exit failures into typed
  `xtask` errors, and verify the worktree status is unchanged afterward.
- Unit-test the preflight and exact subprocess plan with a fake command runner;
  tests must not invoke Git remotes or Codex. Cover missing Codex, fetch
  failure, dirty/untracked files, missing upstream, an unpushed `HEAD`, reviewer
  failure, successful no-findings output, and successful findings output.
- Document Codex installation/authentication as a local prerequisite. CI runs
  deterministic build and test gates only; the authenticated AI review remains
  an explicit post-push developer step.

## Milestone 1: Standalone Workspace Bootstrap

Create the repository foundation used by the AI shared library, adapted for a
sandbox-only Rust workspace. At the end of this milestone, repository tooling
and documentation structure should work without a Juno checkout.

- [x] Copy the shared-library reference's root `AGENTS.md` into this repository.
- [x] Add a root Cargo workspace using resolver 2, Rust edition 2024, MIT
      licensing, `0.2.0` workspace versioning, and `Futex` authorship.
- [x] Add `rust-toolchain.toml` pinned to Rust 1.95.0 with `rustfmt` and
      `clippy`, matching the shared-library reference.
- [x] Add `.cargo/config.toml` with the `cargo xtask` alias, a Rust-focused
      `.gitignore`, and the repository license file.
- [x] Add an `xtask` crate adapted from the AI repository with `check`,
      `rust-file-length-lint --all`, `smoke-test`, and `review` commands; omit
      all AI model, media, MCP, and credentialed-test dependencies.
- [x] Implement and test `review` exactly as specified in Review Implementation,
      including its prompt file, Git preflight, Codex invocation, and typed
      failure paths.
- [x] Align the native review invocation with the installed Codex CLI while
      keeping the repository reporting contract enforced through `AGENTS.md`.
- [x] Unit-test workspace discovery, command planning, and the 300-line Rust
      file-length audit in `xtask`.
- [x] Keep `plans/README.md` linked from the root README and keep this plan in
      its Active section until every milestone is complete.
- [x] Run `cargo metadata --format-version 1 --no-deps` and the targeted
      `xtask` tests to prove the bootstrap workspace is valid.

## Milestone 2: Copy The Provider-Neutral Interface

Copy the contract first so it is independently usable before any provider is
introduced. At the end of this milestone, downstream code can compile and test
against `sandbox-interface` without Juno or E2B.

- [x] Copy the complete sandbox interface source tree from the recorded Juno
      revision into `crates/sandbox-interface`, including `src/_tests_`,
      `tests/backend_conformance`, `tests/read_only_file_contract.rs`,
      `Cargo.toml`, and `README.md`.
- [x] Rename the package to `sandbox-interface` and update every production,
      test, example, and documentation import to `sandbox_interface`.
- [x] Define the pinned `internal-error` Git dependency at workspace scope and
      verify every other interface dependency resolves from crates.io.
- [x] Preserve all provider-neutral DTOs, IDs, errors, limits, traits, mocks,
      registry behavior, read-only capability, image realization, process,
      file, port-ingress, terminal, and screen-stack behavior unchanged while
      replacing product-specific type and field descriptions with neutral terms.
- [x] Preserve the public backend conformance harness so every future concrete
      provider can run the same lifecycle and capability checks.
- [x] Adapt the crate README's Related Docs links to standalone files in this
      repository; do not retain broken relative links into Juno.
- [x] Run `cargo test -p sandbox-interface`, including the named backend
      conformance and read-only file integration-test targets.
- [x] Run package Clippy with warnings denied:
      `cargo clippy -p sandbox-interface --all-targets --all-features -- -D warnings`.

## Milestone 3: Copy E2B As The First Provider

Add the concrete adapter only after the interface passes independently. At the
end of this milestone, E2B should implement the copied interface with the same
credential, lifecycle, reconciliation, file, process, port, and screen behavior
as the recorded Juno source.

- [x] Copy the complete E2B adapter source tree into `crates/sandbox-e2b`,
      including all nested `_tests_`, `tests/live_e2b/`, `tests/live_e2b.rs`,
      `tests/screen_resize_deadline.rs`, `Cargo.toml`, and `README.md`.
- [x] Rename the package to `sandbox-e2b` and update every production, test,
      example, and documentation import to `sandbox_e2b`.
- [x] Add the crate to workspace members and dependencies and resolve
      `sandbox-interface` through the local workspace path.
- [x] Preserve the injected E2B control API and envd process-transport traits,
      keeping provider IDs, access tokens, traffic credentials, payloads, and
      errors inside the adapter boundary.
- [x] Preserve provider-neutral conformance coverage in the E2B test suite,
      along with pagination, ambiguous-delivery, snapshot reconciliation,
      terminal identity/recovery, bounded file/process IO, private port ingress,
      image realization, and screen deadline/resize regression tests.
- [x] Replace hard-coded provider metadata keys and terminal process-tag
      prefixes with neutral, validated configuration while preserving exact
      lookup, fencing, recovery, and reconciliation semantics.
- [x] Make the screen-helper executable path validated adapter configuration;
      use a neutral helper name in defaults, tests, examples, and documentation.
- [x] Preserve `live-e2b` as an opt-in feature and keep every credentialed test
      ignored by default; compiling `--all-features` must not make network calls.
- [x] Document that custom E2B template publication belongs to
      the external template release project, while `E2B_SCREEN_TEMPLATE_ID`
      selects a compatible released template for the optional screen smoke test.
- [x] Run `cargo test -p sandbox-e2b` and the Linux process-group deadline
      integration test without credentials.
- [x] Run package Clippy with warnings denied:
      `cargo clippy -p sandbox-e2b --all-targets --all-features -- -D warnings`.

## Milestone 4: Standalone Documentation And Smoke Coverage

Replace monorepo assumptions with a complete shared-library entry point. At the
end of this milestone, a consumer should be able to choose the interface or
E2B adapter, configure it, and run every safe check from repository docs alone.

- [x] Expand the root README with the workspace purpose, crate map, provider
      architecture, developer setup, credential-free checks, optional live E2B
      commands, and key code entry points.
- [x] Keep both crate READMEs publishable and in the required section order;
      explain that consumers depend on the interface and composition roots
      construct the E2B adapter.
- [x] Add concise standalone protocol documentation for the library-owned
      backend contract and E2B adapter guarantees, distilled from Juno without
      importing application/store orchestration or broken monorepo links.
- [x] Record the exact Juno source commit and the copy-versus-adapt decisions in
      the root documentation for future synchronization audits.
- [x] Implement a credential-free `cargo xtask smoke-test` that constructs
      valid interface values and an E2B backend/configuration with placeholder
      credentials but dispatches no provider request.
- [x] Update `xtask/README.md` for the sandbox workspace and test the smoke
      command's success and failure boundaries.
- [x] Search tracked files for absolute `/Users/...` paths, missing local
      Markdown links, unintended Juno-only crate dependencies, and legacy
      product branding; keep source provenance paths only in this plan.

## Milestone 5: Full Verification

Prove the copied code behaves as one standalone product surface. At the end of
this milestone, every credential-free local gate should pass from a clean
checkout.

- [x] Run `cargo metadata --format-version 1 --no-deps` and `cargo tree` to
      confirm there are no path dependencies outside this repository.
- [x] Run `cargo fmt --all -- --check`; if it fails, format and rerun it.
- [x] Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [x] Run `cargo test --workspace --all-features`; verify ignored live E2B tests
      compile but do not execute.
- [x] Run `cargo xtask rust-file-length-lint --all` and keep every Rust file at
      or below 300 lines without compacting code to evade the limit.
- [x] Run `cargo xtask smoke-test` and then `cargo xtask check` from the root.
- [x] Generate and commit `Cargo.lock`, then rerun the full check with
      `--locked` equivalents where the automation supports them.
- [x] Do not run ignored live tests unless E2B credentials are explicitly
      supplied for this work and billable external calls are intended. If run,
      execute lifecycle cleanup even after failures and record the template IDs
      and outcomes without exposing credentials.

## Milestone 6: GitHub CI

Mirror the safe local gates in GitHub Actions. At the end of this milestone,
pull requests and pushes to `main` should validate the complete standalone
workspace without secrets or live provider usage.

- [x] Add `.github/workflows/ci.yml` modeled on the AI shared library for
      `pull_request` and `main` pushes.
- [x] Install the pinned Rust toolchain and safely cache Cargo registry, Git,
      and build artifacts using manifest and lockfile hashes.
- [x] Run metadata, formatting, workspace Clippy, all-feature tests, the Rust
      file-length lint, the credential-free smoke test, and `cargo xtask check`.
- [x] Ensure CI has no E2B secret dependency and cannot execute ignored live
      tests accidentally.
- [x] Validate the workflow syntax and document the CI contract in the root
      README.

## Milestone 7: Handoff, PR, And Review

Finish the extraction and leave the separate Juno adoption ready for a pinned
revision follow-up. Do not automatically fix review findings without explicit
user direction.

- [x] Prepare a Juno follow-up note showing how its workspace dependencies can
      replace the two local paths with this repository's reviewed Git revision;
      do not modify Juno in this repository's PR.
- [x] Move this plan from Active to Completed in `plans/README.md` only after
      all previous milestones pass.
- [x] Review `git diff origin/main...` for missing copied files, generated
      artifacts, unintended behavior changes, secrets, stale Juno paths, and
      unrelated edits.
- [x] Run `git add -A`, commit the complete work with a Conventional Commit,
      push the current branch, open a PR against `main`, and confirm CI passes.
- [x] After the push, run `cargo xtask review`; confirm its Git preflight and
      worktree-integrity check pass, then report every finding with a number,
      severity, context, impact, lettered solution options, and recommendation,
      without changing the implementation.

## Milestone 8: Review Finding Remediation

Resolve the explicitly approved follow-up review findings without weakening the
provider-neutral contract. At the end of this milestone, credentials cannot
cross redirects or attacker-shaped routes, terminal mutations use atomic
provider identity, validated configuration stays immutable, handled profile
errors remain typed, and failed writes clean their temporary files.

- [x] Disable redirects for every HTTP client that carries a provider
      credential, with regression coverage.
- [x] Construct and validate the exact envd URL before attaching an access
      token, for both process and file requests.
- [x] Address terminal input and close operations by their unique provider tag
      in one request so PID reuse cannot retarget them.
- [x] Make validated adapter configuration externally immutable and expose
      read-only accessors where callers need them.
- [x] Return the provider-neutral `UnknownProfile` error before provider
      dispatch when sandbox creation names an unconfigured profile.
- [x] Remove upload staging and destination-temporary files after every failed
      replacement-write attempt.
- [x] Update the adapter and protocol documentation for the tightened safety
      guarantees.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, the file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for prohibited legacy terms and review the complete
      branch diff for secrets, artifacts, and unrelated edits.
- [x] Commit and push the fixes, confirm GitHub CI, then run a clean post-push
      `cargo xtask review` without changing the worktree.

## Milestone 9: Final Review Finding Remediation

Resolve every finding from the post-push review without replaying completed
image builds or weakening the provider-neutral byte and validation contracts.

- [x] Split successful image completion from idempotent source-sandbox cleanup
      so a cleanup failure cannot replay setup and verification commands.
- [x] Reject oversized file writes before connecting to or resuming a provider
      sandbox.
- [x] Preserve definitive provider rejection statuses without waiting for an
      unused response body or treating the mutation as delivery-ambiguous.
- [x] Enforce arbitrary terminal transcript limits as exact byte counts.
- [x] Move the smoke-test bodies into the repository-required `_tests_` tree.
- [x] Add focused regression coverage for all behavioral findings and update
      the public contract and adapter documentation.
- [x] Run formatting, focused regressions, Clippy, the full workspace test
      suite, the file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for prohibited legacy terms, secrets, artifacts, and
      unrelated edits.
- [x] Commit and push the fixes, confirm GitHub CI, then run a clean post-push
      `cargo xtask review` without changing the worktree.

## Milestone 10: Recovery And Process Safety Review Remediation

Resolve every finding from the second clean post-push review. At the end of
this milestone, image realization can be recovered without losing provider
state or replaying completed build steps, and provider process uncertainty can
never be reported as successful completion.

- [x] Preserve the image source sandbox when snapshot reconciliation remains
      unresolved, and return enough typed context for the caller to recover it.
- [x] Look for a completed correlated image before staging files or rerunning
      user-authored image setup and verification commands.
- [x] Require normal process termination before any regular-file helper or
      restored-sandbox maintenance helper reports success.
- [x] Kill restored terminal processes by their stable provider tag rather
      than a separately observed numeric PID.
- [x] Replace the fixed failed-write cleanup delay with a writer revocation
      fence checked immediately before atomic replacement.
- [x] Stop configured image helper processes with bounded TERM/KILL escalation
      and verify that they are gone before size measurement and snapshotting.
- [x] Classify transient envd request and response failures as provider
      unavailability, while preserving ambiguous delivery for mutations.
- [x] Add focused regression coverage and update the public interface, adapter,
      adoption, and protocol documentation for every changed contract.
- [x] Run formatting, focused regressions, Clippy, the full workspace test
      suite, the file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for prohibited legacy terms, secrets, artifacts, and
      unrelated edits.
- [x] Commit and push the fixes, confirm GitHub CI, then run a clean post-push
      `cargo xtask review` without changing the worktree.

## Milestone 11: Durable Image And Write Recovery

Resolve every finding from the third clean post-push review. At the end of this
milestone, callers durably control image source creation and snapshot dispatch,
provider recovery failures retain their evidence, and file and transport
reconciliation report only outcomes that are proven durable.

- [x] Replace monolithic image realization with explicit caller-driven source,
      preparation, snapshot-dispatch, and recovery phases so retries never
      allocate a second source or replay completed build side effects.
- [x] Retain image source sandboxes whenever preparation or recovery cannot
      prove a safe terminal outcome.
- [x] Reconcile nonzero writer exits through the same commit-or-revoke outcome
      mapping used for uncertain transport failures.
- [x] Preserve definitive pre-commit writer rejections while reconciling only
      exit paths that could have claimed or completed the replacement.
- [x] Classify control-plane response-body transport failures as retryable
      provider unavailability.
- [x] Sync the destination directory before cleanup reports an already visible
      replacement as durably committed.
- [x] Add failing regressions first for all six review findings, then update the
      interface, adapter, conformance, adoption, and protocol documentation.
- [x] Run formatting, focused regressions, Clippy, the full workspace test
      suite, the file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for prohibited legacy terms, secrets, artifacts, and
      unrelated edits.
- [x] Commit and push the fixes, confirm GitHub CI, then run a clean post-push
      `cargo xtask review` without changing the worktree.

## Milestone 12: Final Integrity And Conformance Remediation

Resolve every finding from the fourth clean post-push review. At the end of
this milestone, file replacement proves the staged payload identity, inherited
credential helpers are confirmed stopped, credentialed live probes never
follow redirects, and the public conformance harness accepts every outcome
allowed by its own interface without leaking resources.

- [x] Bind writer and cleanup reconciliation to the requested payload digest,
      with a failing same-size staging-tamper regression added first.
- [x] Stop inherited drive helpers with bounded TERM/KILL escalation and fail
      cleanup unless their exit is confirmed, with regression coverage added
      first for both restored cleanup and image preparation.
- [x] Disable redirects for every opt-in live client that attaches an E2B
      traffic credential.
- [x] Recover valid asynchronous image snapshot outcomes in the conformance
      harness while preserving bounded execution and source cleanup.
- [x] Accept omitted retained-source diagnostics in the conformance harness,
      validate them when present, and always clean up the caller-known source.
- [x] Update the public contract, adapter documentation, and crate READMEs for
      the tightened integrity, cleanup, and conformance guarantees.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, the file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for prohibited legacy terms, secrets, artifacts, and
      unrelated edits.
- [x] Commit and push the fixes, confirm GitHub CI, then run a clean post-push
      `cargo xtask review` without changing the worktree.

## Milestone 13: Path Safety And Conformance Completion

Resolve every finding from the fifth clean post-push review. At the end of this
milestone, terminal storage cannot be redirected through sandbox-owned
symlinks, every snapshot probe accepts asynchronous provider completion, image
conformance cleans all created sources, and malformed safe responses remain
retryable provider failures.

- [x] Replace restored-terminal cleanup and log-directory creation with
      descriptor-relative, non-following helpers, with symlink-escape
      regressions added first.
- [x] Reuse bounded snapshot recovery in the main conformance flow, with a
      valid asynchronous-backend regression added first.
- [x] Funnel every post-create image conformance exit through source cleanup,
      with preparation, validation, and inventory failure regressions added
      first.
- [x] Classify malformed successful control reads and safe envd responses as
      provider unavailability while preserving ambiguous mutation outcomes,
      with regressions added first.
- [x] Update the public contract, adapter documentation, and crate READMEs for
      the completed path-safety, cleanup, and retry guarantees.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, the file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for prohibited legacy terms, secrets, artifacts, and
      unrelated edits.
- [x] Commit and push the fixes, confirm GitHub CI, then run a clean post-push
      `cargo xtask review` without changing the worktree.

## Milestone 14: Validation, Redaction, And Recovery Pacing

Resolve every finding from the sixth clean post-push review. At the end of this
milestone, malformed process requests cannot reach the provider, image command
diagnostics cannot expose known connection identifiers or credentials, and
snapshot recovery gives asynchronous providers a bounded real-time window
without bursting their APIs.

- [x] Add failing regressions first for pre-dispatch process validation, image
      diagnostic redaction, and one-second snapshot recovery pacing.
- [x] Validate process command text, combined argv bytes, stream bounds, and
      deadlines before acquiring a provider connection.
- [x] Redact the provider sandbox reference and envd access token from failed
      image command output before returning a handled diagnostic.
- [x] Redact overlapping sensitive diagnostic values longest-first so a
      shorter provider value cannot leave part of a longer credential visible.
- [x] Pace in-progress snapshot recovery through an injected sleeper while
      keeping conformance tests deterministic and the recovery window bounded.
- [x] Update the public contract, adapter documentation, and crate READMEs for
      the validation, redaction, and recovery timing guarantees.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, the file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for prohibited legacy terms, secrets, artifacts,
      whitespace errors, and unrelated edits.
- [x] Commit and push the fixes, confirm GitHub CI, then run a clean post-push
      implementation review without changing the worktree.
- [ ] After a clean review, record plan completion and move this plan from
      Active to Completed in `plans/README.md`.

## Milestone 15: Boundary And Recovery Review Remediation

Resolve every finding from the seventh clean post-push review. At the end of
this milestone, bounded diagnostics redact secrets before truncation, image
requests fail validation before touching a provider, snapshot recovery accepts
eventually consistent inventory without losing cleanup handles, and malformed
multi-channel process events fail closed.

- [x] Add failing regressions first for boundary-spanning secret redaction,
      image-input preflight, recover-only conformance polling, ambiguous live
      snapshot recovery, and multi-channel process data rejection.
- [x] Redact complete sensitive values before applying the final bounded image
      diagnostic tail, including values that cross the tail boundary.
- [x] Validate every image input file size before acquiring provider access or
      writing any earlier input.
- [x] Pace and bound the conformance harness's recover-only snapshot check so
      eventually consistent providers are accepted.
- [x] Retain and recover the live lifecycle snapshot request after in-progress
      or delivery-ambiguous creation so cleanup can track the created snapshot.
- [x] Reject malformed process data events unless exactly one output channel is
      present.
- [x] Update the public contract, adapter documentation, and crate READMEs for
      every clarified guarantee.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, the file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for prohibited legacy terms, secrets, artifacts,
      whitespace errors, and unrelated edits.
- [ ] Commit and push the fixes, confirm GitHub CI, then run a clean post-push
      implementation review without changing the worktree.
- [ ] After a clean review, record plan completion and move this plan from
      Active to Completed in `plans/README.md`.

## Milestone 16: Trusted Shell And Cleanup Recovery

Resolve every finding from the eighth clean post-push review. At the end of
this milestone, user-controlled login profiles cannot bypass trusted image or
terminal-wrapper setup, and every conformance or live-test sandbox creation
retains enough identity to recover and destroy an uncertain or partially
initialized provider resource.

- [x] Add failing regressions first for login-profile isolation, transcript
      wrapper ordering, conformance source-create recovery, live create-request
      retention, and restored-sandbox tracking before terminal cleanup.
- [x] Run trusted image commands and size measurement without loading
      user-controlled login profiles.
- [x] Start the trusted terminal transcript wrapper before the one intended
      interactive login shell loads the user's profile.
- [x] Recover uncertain conformance image-source creation with bounded,
      one-second-paced recover-only polling and destroy any recovered source.
- [x] Centralize live-test sandbox creation so its exact request is retained
      before dispatch, uncertain creation is recovered, and every returned
      provider sandbox is tracked before later work can fail.
- [x] Update the public contract, adapter documentation, and crate READMEs for
      the clarified trusted-shell and cleanup-ownership guarantees.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, the file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for prohibited legacy terms, secrets, artifacts,
      whitespace errors, and unrelated edits.
- [ ] Commit and push the fixes, confirm GitHub CI, then run a clean post-push
      implementation review without changing the worktree.
- [ ] After a clean review, record plan completion and move this plan from
      Active to Completed in `plans/README.md`.

## Milestone 17: Storage, Cleanup, And Identity Remediation

Resolve every finding from the ninth post-push review. At the end of this
milestone, terminal transcripts remain bound to verified storage, conformance
always recovers and cleans provider resources, invalid requests fail before
provider access, and backend inventory preserves each sandbox consumer class.

- [x] Add failing regressions first for descriptor-bound terminal transcript
      access, every conformance create and cleanup path, recovery-profile
      validation, transfer-path preflight, and consumer-class metadata.
- [x] Bind terminal transcript creation, writing, and reading to non-following
      descriptor-relative helpers so sandbox-controlled path replacement cannot
      redirect transcript bytes.
- [x] Route every conformance sandbox create through shared bounded,
      one-second-paced recovery and track resources before later work can fail.
- [x] Run unconditional best-effort conformance cleanup for every tracked
      terminal, snapshot, and sandbox while preserving the primary failure.
- [x] Share sandbox-create validation with recovery so unknown profiles and
      unsupported network policies fail before provider dispatch.
- [x] Validate trusted roots and relative transfer paths before reads or writes
      acquire provider access.
- [x] Carry `SandboxConsumer` through backend creation, provider metadata, and
      managed inventory, including compatibility behavior for older resources.
- [x] Update the public contract, adapter documentation, and crate READMEs for
      the completed storage, cleanup, validation, and identity guarantees.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, the file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for prohibited legacy terms, secrets, artifacts,
      whitespace errors, and unrelated edits.
- [ ] Commit and push the fixes, confirm GitHub CI, then run a clean post-push
      implementation review without changing the worktree.
- [ ] After a clean review, record plan completion and move this plan from
      Active to Completed in `plans/README.md`.

## Milestone 18: Trusted Python Helper Isolation

Resolve the hostile-module import found while reviewing the storage and
cleanup fixes. At the end of this milestone, sandbox-owned working directories
and Python environment settings cannot run code before any trusted adapter
helper.

- [x] Record the review finding and add a regression that places hostile
      standard-library module names in the helper working directory and
      `PYTHONPATH`.
- [x] Run every regular-file, terminal-storage, restored-cleanup, and terminal
      transcript Python helper in isolated mode without site initialization.
- [x] Update the public contract, adapter documentation, and crate README for
      the trusted Python startup guarantee.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, the file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for prohibited legacy terms, secrets, artifacts,
      whitespace errors, and unrelated edits.
- [ ] Commit and push the fix, confirm GitHub CI, then run a clean post-push
      implementation review without changing the worktree.
- [ ] After a clean review, record plan completion and move this plan from
      Active to Completed in `plans/README.md`.

## Milestone 19: Final Routing And Resource Remediation

Resolve every finding from the post-isolation review. At the end of this
milestone, opaque provider identifiers cannot alter authenticated control
routes, invalid image inputs cannot partially mutate a source, image size
measurement fails closed, completed writes do not accumulate state markers,
and incremental file-length checks inspect only relevant Rust files.

- [x] Record all five review findings and add failing regressions first for
      control-route dot segments, image-path preflight, failed size
      measurement, completed-write marker cleanup, and incremental file
      selection.
- [x] Reject `.` and `..` provider identifiers before building any E2B control
      route.
- [x] Validate every staged image file's path and size before acquiring
      provider access or writing an earlier file.
- [x] Preserve `du` failures so incomplete image-size totals cannot be
      accepted as successful measurements.
- [x] Remove a replacement write's state marker after the writer has confirmed
      normal completion, without removing fences for uncertain writers.
- [x] Honor the file-length linter's default incremental scope through the
      injected command-runner boundary while keeping `--all` as a full scan.
- [x] Update the public contract, adapter documentation, and crate READMEs for
      the tightened routing, preflight, measurement, and cleanup guarantees.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, the file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for prohibited legacy terms, secrets, artifacts,
      whitespace errors, and unrelated edits.
- [ ] Commit and push the fixes, confirm GitHub CI, then run a clean post-push
      implementation review without changing the worktree.
- [ ] After a clean review, record plan completion and move this plan from
      Active to Completed in `plans/README.md`.

## Milestone 20: Workload Isolation And Safe Cache Cleanup

Resolve every finding from the final resource review. At the end of this
milestone, sandbox workloads cannot remove uncertain-write fences or replace
terminal transcript storage, and image cleanup cannot traverse a
setup-created symlink outside the intended cache paths.

- [x] Record the three review findings for workload-controlled write markers,
      workload-controlled transcript files, and symlinked image-cache parents.
- [x] Add failing regressions first for trusted process authentication,
      private write-fence placement, privilege-separated transcript capture,
      and descriptor-relative cache deletion.
- [x] Run replacement writers and reconcilers as the trusted sandbox account
      and keep their atomic state markers in a private trusted directory.
- [x] Preserve workload ownership on replacement files even though the writer
      uses the trusted account for private fence access.
- [x] Keep the verified temporary inode trusted until its atomic rename and
      make reconciliation repair workload ownership after an interrupted commit.
- [x] Run the terminal recorder as the trusted account, drop only the
      interactive login shell to the configured workload account, and keep
      transcript creation and reads inside private trusted storage.
- [x] Authenticate recorder discovery, input, and shutdown as the same trusted
      account that owns the recorder process.
- [x] Make transcript byte-limit regressions independent of the CI runner's
      login profile while retaining production login-shell assertions.
- [x] Replace shell cache deletion with a descriptor-relative helper that
      refuses symlinked parents and never follows child symlinks.
- [x] Update the public contract, adapter documentation, and crate README for
      the new account, storage, and cache-cleanup guarantees.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, the file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for prohibited legacy terms, secrets, artifacts,
      whitespace errors, and unrelated edits.
- [ ] Commit and push the fixes, confirm GitHub CI, then run a clean post-push
      implementation review without changing the worktree.
- [ ] After a clean review, record plan completion and move this plan from
      Active to Completed in `plans/README.md`.
