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
- [ ] Commit and push the fixes, confirm GitHub CI, then run a clean post-push
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
- [ ] Commit and push the fixes, confirm GitHub CI, then run a clean post-push
      `cargo xtask review` without changing the worktree.
