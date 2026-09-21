# Copy Sandbox Crates From Juno

## Summary

Copy the provider-neutral sandbox contract crate and its first concrete
provider adapter (E2B) from the Juno monorepo into this standalone workspace,
and make the result build, test, document, and verify itself independently of
Juno.

Source:

- Repository: `futex-ai/firna` (private), revision
  `64e6dd1c1fcc0d44c73fa1b31879cb4ba80448c6` (`main`, 2026-09-21). The local
  checkout `/Users/calummoore/projects/futex/juno` is at the same revision and
  has no local changes under the copied paths.
- Precedent: `futex-ai/ai` (local `/Users/calummoore/projects/futex/ai`) was
  extracted from Juno the same way; its plan
  `plans/copy-ai-crates-from-juno.md`, root `Cargo.toml`, `xtask`, and
  `.github/workflows` are the reference layout for this repository.

Crates copied and renamed:

| Juno crate | New crate | Purpose |
|---|---|---|
| `fna-sandbox-interface` | `sandbox-interface` | Provider-neutral DTOs, states, typed errors, `SandboxBackend`, `SandboxReadOnly`, `SandboxBackendRegistry`, `SandboxService` traits, unimock mocks, and the backend conformance harness |
| `fna-sandbox-e2b` | `sandbox-e2b` | E2B control-plane, envd Connect, and process transport adapter implementing `SandboxBackend` and `SandboxReadOnly` |

Not copied:

- `fna-sandboxes` (authorization, policy, persistence orchestration,
  transcript normalization, recovery, and cleanup). It depends on Juno's
  `fna-store-interface` Postgres contract and stays in Juno as the
  `SandboxService` implementation.
- Juno composition roots (`fna-worker-bin`, `fna-serve-router-bin`,
  `fna-preview-router-bin`) that construct `E2bSandboxBackend`.
- Juno's E2B template fixtures under `infra/e2b`. Templates are released from
  `futex-ai/firna-envs`; this repository documents the helper contract only.

External dependencies already resolved by the copied manifests: `async-trait`,
`base64`, `bytes`, `chrono`, `futures-core`, `futures-util`, `reqwest`,
`serde`, `serde_json`, `strip-ansi-escapes`, `thiserror`, `tokio`, `tracing`,
`unimock`, `url`, `uuid`, plus dev-only `tempfile`. The only workspace-level
support dependency is `internal-error`, consumed as a git dependency at the
same revision Juno and `ai` use
(`6477922a1c4db8c3e35189af69f0147464004b99`). Neither crate uses `json-http`.

## Naming And Compatibility Decisions

- Crate and module names drop the `fna-` prefix: `sandbox_interface::…` and
  `sandbox_e2b::…`. Error prefixes become `[sandbox_interface/error]` and
  `[sandbox_e2b/error]`. Both crates.io names were unregistered on
  2026-09-21.
- Provider-facing wire identifiers are compatibility contracts with released
  E2B templates and with provider resources that Juno deployments already own.
  They are copied unchanged and documented as such: sandbox metadata keys
  (`firna_sandbox_id`, `firna_operation_id`, `firna_workspace_id`,
  `firna_agent_id`, `firna_owner_kind`, `firna_deployment_id`), the
  `firna-terminal-<terminal_id>` process tag, the `firna-operation-…` snapshot
  correlation name, the `/usr/local/bin/firna-screen` helper, and the
  `firna-drive-` / `/drives/me` / `/tmp/firna-drive` scrub targets.
- Prose that describes Juno as the owner ("Firna UUIDv7 handles", "Firna
  worker") is rewritten in crate READMEs and protocol docs as the consumer or
  host application. Literal identifiers above are never rewritten.
- `AGENTS.md` is copied byte-for-byte from Juno (sha256
  `3dc74064217b0bc4183cb053e6b2aa0a28234d49203d50457b0f991fb767050f`), with
  `CLAUDE.md` as a symlink, so this repository follows the same agent rules,
  including the `docs/implementation-review-prompt.md` review flow instead of
  `cargo xtask review`.

## Out Of Scope

- Porting Juno's additional `xtask` lints (ast-grep import rules, `map_err`
  lint, trait audit, source-layout lint). This repository starts with the same
  `xtask` surface as `ai`: `check`, `rust-file-length-lint`, and `smoke-test`.
  A separate plan may add those lints later.
- Gating `unimock` mocks behind a `test-support` feature as `ai-interface`
  does. The copied crates ship mocks unconditionally; changing that is a
  separate, opt-in follow-up because every Juno consumer would need a
  dev-dependency feature update.
- Switching Juno to consume these crates from git. That is Juno work and is
  recorded in the post-merge section below.

## Milestone 1: Repository Guide And Protocol Docs

Establish the agent rules and the normative contract before any Rust lands.
At the end of this milestone the repository contains the copied guide, the
review prompt, a root README, and protocol docs that fully define the two
crates' behavior so implementation needs no guesswork. Juno's
`docs/protocol/agent-sandboxes.md` is the source for every section below; only
the crate-owned contract moves here, while service, store, quota, lease,
cleanup, browser-session, and screen-streaming router rules stay in Juno.

- [x] Copy `AGENTS.md` from Juno at the pinned revision and add the
      `CLAUDE.md -> AGENTS.md` symlink.
- [x] Copy `docs/implementation-review-prompt.md` from Juno and retarget its
      repository sentence to this workspace.
- [ ] Review the copied `AGENTS.md` for bullets that name Juno-only surfaces
      (`cargo xtask check --include fna-cli`, `fna-db`, `ts/app`, `ts/web`,
      `@firna/ui`, `firna-apps`, `docs/mockups`) and confirm with the user
      whether to keep them verbatim (the `ai` repository kept its
      Juno-derived rules) or trim them. Never remove a general rule.
- [ ] Write `docs/protocol/sandbox-backend-contract.md` covering: scope and
      ownership (`ResourceOwner` platform-or-agent ownership, consumer-owned
      UUIDv7 handles, opaque `ProviderRef`); the mandatory backend capability
      list (create/inspect/list/connect/pause/destroy, snapshot
      create/inspect/delete and restore, terminal create/recover/input/output/
      inspect/close, at-most-once snapshot recovery, non-allocating terminal
      recovery, disconnected transcript recovery, `realize_image`,
      `read_file`/`write_file`, `run_process`, `port_ingress`,
      `ensure_screen_stack`/`resize_screen_stack`, and the three
      `SandboxReadOnly` operations); the rule that optional capability flags
      and provider downcasts are forbidden; the existing-resource identity
      echo rule; sandbox, snapshot, terminal, action, and cleanup state enums;
      `SandboxConsumer` (`runtime`, `browser`) and `SandboxNetworkPolicy`
      (`open` only); every constant the interface owns (256 MiB file
      transfer, path byte cap, 4 KiB image-command output tail, 120-byte and
      32-item profile bounds and name grammar, process-run argv/stream/
      deadline caps, viewport bounds 320..=3840 by 240..=2160 and at most
      8,294,400 pixels); the typed error enum with `Internal(InternalError)`
      fallback; the `SandboxService` trait as a consumer-implemented boundary;
      the conformance harness as the admission gate for new providers; and
      the compatibility-identifier list from the decisions section.
- [ ] Write `docs/protocol/e2b-sandbox-provider.md` covering: adapter
      configuration validation (`E2bAdapterConfig`, `E2bProfile`, HTTPS
      origin-only API base, nonblank canonical values, deny-list
      canonicalization and deduplication, 32-profile bound); control requests
      (`secure: true`, `allowPublicTraffic: false`, auto-pause, 10/30/60 s
      timeouts, delivery-ambiguous mutations versus retryable reads and
      deletes, cursor pagination limits, sandbox ID echo validation, ignored
      deprecated domain); envd Connect framing and process transport (PTY
      through `util-linux script`, provider-log offsets, 2 MiB cap supplied
      by the caller, absolute deadline of wait plus five seconds, empty-chunk
      retry at end of file); terminal identity (PID plus tag) and recovery by
      tag listing; snapshot correlation naming and the 60-second
      source-scoped reconciliation poll; image realization composition and
      scrub steps; bounded file reads through the non-following descriptor
      helper and `realpath`-validated writes; read-only exec through
      `GET /sandboxes/{id}` requiring `running` and `autoResume=false`; port
      ingress upstream and traffic-token header; screen ensure, capability
      probe, and resize dispatch with 4 KiB/64 KiB bounds, `/usr/bin/timeout
      --signal=KILL`, and `ScreenViewportResizeUnconfirmed`; and the adapter
      error enum mapping to interface errors.
- [ ] Write `docs/protocol/screen-stack-helper.md` covering the template
      helper contract: argv-direct `firna-screen ensure` semantics (probe
      loopback 6080 and 6081, start only missing processes, idempotent exit
      zero), the version-1 `capabilities` JSON envelope and exact bound set,
      the `resize <width> <height>` acknowledgment, 15-second execution cap,
      no shell, and that templates are released from `futex-ai/firna-envs`.
- [ ] Write `docs/protocol/live-e2b-tests.md` covering the credentialed
      suite: `E2B_API_KEY`, `E2B_TEMPLATE_ID` (default `base`),
      `E2B_SCREEN_TEMPLATE_ID`; the lifecycle, private port ingress, and
      screen bridge tests; cleanup ordering (terminals, sandboxes, then
      snapshot) on success or failure; the credential-free compile guard; the
      CI gating and secret boundary; and the rule that provider ids, tokens,
      and response bodies are never recorded.
- [ ] Write the root `README.md` with purpose, crate map, protocol links,
      developer get-started commands, key code entry points, and the
      `plans/README.md` link. Commands may reference the `xtask` surface
      delivered in Milestone 2.
- [ ] Validate the Markdown: every relative link resolves, each protocol doc
      stays near 250 lines, and no statement conflicts with the sections that
      remain in Juno's `agent-sandboxes.md`.

## Milestone 2: Workspace Bootstrap

Create a valid Cargo workspace with the same automation shape as `ai` before
any crate is copied. At the end of this milestone `cargo metadata`,
`cargo xtask check`, and `cargo xtask rust-file-length-lint --all` succeed on
an `xtask`-only workspace.

- [ ] Add root `Cargo.toml` with `resolver = "2"`, workspace package metadata
      (`edition = "2024"`, `version = "0.1.0"`, `license = "MIT"`,
      `authors = ["Futex"]`), members `xtask` plus the two crate paths, and
      workspace dependencies for `internal-error` (git, pinned revision),
      `sandbox-interface`, and `sandbox-e2b`.
- [ ] Add `rust-toolchain.toml` pinning `1.95.0` with `clippy` and `rustfmt`.
- [ ] Add `.cargo/config.toml` with the `xtask = "run -p xtask --"` alias.
- [ ] Add `.gitignore` entries for `/target/`, `/.conductor/`, `.DS_Store`,
      and `*.log`.
- [ ] Copy the `xtask` crate skeleton from `ai` (`cli.rs`, `check.rs`,
      `command.rs`, `error.rs`, `file_length.rs`, `workspace.rs`, `main.rs`,
      `_tests_`, `README.md`) with only the `check` and
      `rust-file-length-lint` commands; drop `review.rs` and the AI-crate
      smoke modules. `smoke-test` is added in Milestone 4 once there is a
      backend to construct.
- [ ] Write the `xtask/README.md` in the required crate README section order.
- [ ] Run `cargo metadata --format-version 1 --no-deps`, `cargo xtask check`,
      and `cargo xtask rust-file-length-lint --all`.
- [ ] Commit the generated `Cargo.lock`.

## Milestone 3: Copy And Rename Crates

Bring both crates across without changing behavior, then rename the crate
identity. At the end of this milestone the renamed sources, tests, fixtures,
and crate READMEs are tracked and `cargo metadata` resolves them.

- [ ] Fetch `crates/fna-sandbox-interface` and `crates/fna-sandbox-e2b` from
      `futex-ai/firna` at the pinned revision (for example
      `gh api repos/futex-ai/firna/tarball/<rev>` or a sparse clone) into
      `crates/sandbox-interface` and `crates/sandbox-e2b`, preserving every
      `_tests_` directory, `tests/` tree, `README.md`, `[[test]]` manifest
      entry, and the `live-e2b` feature.
- [ ] Rename package names in both manifests; replace
      `fna-sandbox-interface.workspace = true` with
      `sandbox-interface.workspace = true`; keep `internal-error.workspace`.
- [ ] Rewrite every `fna_sandbox_interface::` and `fna_sandbox_e2b::` path in
      `src`, `_tests_`, and `tests` to the new crate names.
- [ ] Rewrite the `#[error("[fna_sandbox_interface/error] …")]` and
      `#[error("[fna_sandbox_e2b/error] …")]` prefixes to the new crate names.
- [ ] Update crate-level doc comments that mention Firna ownership without
      touching compatibility identifiers.
- [ ] Add a regression test in `crates/sandbox-e2b` that asserts the
      compatibility identifiers (terminal tag prefix, snapshot correlation
      name prefix, metadata keys, screen helper path, drive scrub targets)
      keep their exact `firna` literals, so a future cleanup cannot break
      deployed Juno resources or released templates.
- [ ] Update both crate READMEs: new names, Quick Start examples that compile
      against the public API, Development commands, Key Code, and Related
      Docs links pointing at this repository's protocol docs; rewrite Juno
      ownership prose as consumer-neutral prose.
- [ ] Add both crates to workspace members and confirm no `fna-` crate name
      or Juno-relative doc path remains under `crates/` and `xtask/`:

      ```sh
      grep -rnE "fna_sandbox|fna-sandbox|agent-sandboxes\.md" crates xtask
      ```

      The command must print nothing.
- [ ] Run `cargo metadata --format-version 1 --no-deps`.

## Milestone 4: Compile, Test, And Smoke Test

Prove the copied crates work as a standalone product surface. At the end of
this milestone formatting, Clippy, unit and integration tests, the file-length
lint, the credential-free smoke test, and `cargo xtask check` all pass.

- [ ] Run `cargo fmt --all -- --check`; if it fails, run `cargo fmt --all` and
      re-run the check.
- [ ] Run `cargo clippy --workspace --all-targets --all-features` and fix
      every warning.
- [ ] Run `cargo test --workspace --all-features`, including the Linux-only
      `screen_resize_deadline` process smoke that needs `/usr/bin/timeout` and
      `/usr/bin/python3`, and the loopback HTTP control-client tests.
- [ ] Run `cargo xtask rust-file-length-lint --all`; split any copied file
      that exceeds 300 lines.
- [ ] Add the `smoke-test` xtask command: build an `E2bAdapterConfig` with a
      placeholder key, an HTTPS origin, one profile with a deny list, and the
      900-second idle timeout; construct `E2bSandboxBackend` and erase it to
      `DynSandboxBackend`; validate a `ScreenViewportSize` and a profile name
      through the interface constants. No network calls and no credentials.
- [ ] Wire `smoke-test` into `cargo xtask check` after the file-length lint.
- [ ] Run the credential-free live-suite compile guard:
      `cargo test -p sandbox-e2b --features live-e2b --test live_e2b` must
      compile and report the three tests as ignored.
- [ ] If the user supplies `E2B_API_KEY`, run the lifecycle and private port
      ingress live tests once against the `base` template and record the
      outcome in the PR; otherwise state that live coverage ran only through
      the compile guard.
- [ ] Run `cargo xtask check` until it passes.

## Milestone 5: GitHub CI Workflows

Add repository CI so every pull request runs the same checks as `cargo xtask
check`, and add a credentialed E2B workflow that never runs from forks. At the
end of this milestone both workflows are present and documented.

- [ ] Add `.github/workflows/ci.yml` for `pull_request` and `push` to `main`
      that installs Rust 1.95.0, caches Cargo, and runs `cargo metadata`,
      `cargo fmt --all -- --check`,
      `cargo clippy --workspace --all-targets --all-features`,
      `cargo test --workspace --all-features`,
      `cargo xtask rust-file-length-lint --all`, `cargo xtask smoke-test`, and
      `cargo xtask check`.
- [ ] Add `.github/workflows/live-e2b.yml` modeled on `ai`'s
      `live-models.yml`: same-repository non-Dependabot pull requests, a daily
      schedule, and manual dispatch; `max-parallel: 1`; a lifecycle job and a
      port-ingress job that fail early when the `E2B_API_KEY` secret is
      missing; and a screen-bridge job that runs only when the
      `E2B_SCREEN_TEMPLATE_ID` repository variable is set. Each job runs
      `cargo test --locked -p sandbox-e2b --features live-e2b --test live_e2b
      <test> -- --ignored --exact --nocapture`.
- [ ] Add a credential-free workflow guard test (as `ai` does for its live
      suites) asserting the workflow names, test selectors, secret names, and
      fork gating so drift is caught without provider calls.
- [ ] Document both workflows in the root `README.md` CI section and in
      `docs/protocol/live-e2b-tests.md`.

## Milestone 6: Documentation, Commit, PR, And Review

Finish with accurate docs, a pushed branch, a pull request, and an independent
review. Review findings are reported, not auto-fixed.

- [ ] Update the root `README.md` with the final feature summary, interface
      list, developer commands, key code entry points, CI description, and
      plan link.
- [ ] Re-read both crate READMEs and all four protocol docs against the
      final code and fix any drift.
- [ ] Move this plan from active to completed in `plans/README.md`.
- [ ] Review `git diff origin/main...` for unrelated changes, generated
      artifacts, missing files, and stale Juno paths.
- [ ] Run `cargo xtask check`.
- [ ] Run `git add -A`, commit with a Conventional Commit message, and push
      the branch.
- [ ] Create a GitHub pull request against `main` and confirm the CI and
      live-E2B workflows start and report their status.
- [ ] Review the complete local diff against `origin/main` using
      `docs/implementation-review-prompt.md`; report each finding with
      severity, context, impact, lettered options, and a recommendation
      without changing the implementation.

## Post-merge follow-up (non-blocking)

- Configure the `E2B_API_KEY` Actions secret and the
  `E2B_SCREEN_TEMPLATE_ID` repository variable in `futex-ai/sandbox`, then
  confirm the first scheduled live run.
- Open a Juno plan to adopt the published crates: add `sandbox-interface` and
  `sandbox-e2b` git dependencies at the merged revision, rewrite imports in
  the dependents (`fna-sandboxes`, `fna-browser`, `fna-browser-interface`,
  `fna-browser-store-pg`, `fna-deployments`, `fna-deployments-interface`,
  `fna-deployments-store-pg`, `fna-env-pages`, `fna-envs`,
  `fna-envs-interface`, `fna-envs-store-pg`, `fna-fns`, `fna-git-store-pg`,
  `fna-agent-tools`, `fna-core`, `fna-preview-router`,
  `fna-preview-router-bin`, `fna-previews-store-pg`, `fna-router-proxy`,
  `fna-serve-router`, `fna-serve-router-bin`, `fna-server-bin`,
  `fna-store-interface`, `fna-store-pg`, `fna-worker-bin`), delete the two
  local crates, link `docs/protocol/agent-sandboxes.md` to this repository's
  protocol docs, and extend Juno's `futex_ai_revision_audit` to cover the new
  git dependency.
- Decide whether to gate `unimock` behind a `test-support` feature in both
  crates to match `ai-interface`.
