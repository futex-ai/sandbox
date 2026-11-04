# Sandbox Capabilities And Boat

Status: Active. Branch: `conductor/ciproxy-mvp-milestone-12-sandbox-capabilities-and-boat`.
[PR #11](https://github.com/futex-ai/sandbox/pull/11) is open into `main`.
This plan stays in the pull request at the owner's request.

## Summary

Split the required sandbox backend into independent capabilities. Add the
capacity, start-refusal, and provider lifetime contracts needed by ciproxy.
Add a Boat adapter for lifecycle, saved snapshots, processes, and files.
Keep E2B behavior and its existing guarantees. Move every in-repository
consumer, mock, conformance probe, and live test to the capability contracts.

The owner approved this approach on 4 November 2026 as the sandbox part of
ciproxy MVP Milestone 12. The owner chose this repository for the Boat adapter
instead of a private adapter inside ciproxy. Breaking interface changes are
approved. Juno adopts the split in a separate change after this pull request.
This change does not edit Juno or ciproxy. It makes no real Boat or E2B calls.

Contract owners:

- [Sandbox contract](../docs/sandbox-contract.md).
- [E2B adapter](../docs/e2b-adapter.md).
- [Juno adoption](../docs/juno-adoption.md).
- [Interface crate](../crates/sandbox-interface/README.md).

## Design

Products depend on the capabilities that they use. Providers implement only
the capabilities that they support. The split covers lifecycle, resumable
lifecycle where needed, saved snapshots, image preparation, processes, files,
terminals, port ingress, and screens. There is no required all-capability
implementation for a provider such as Boat. Preserve all 28 existing methods
in the appropriate capability. Replace monolithic mocks and routing with
explicit capability dependencies.

The create request can select a provider size or a resource request containing
vCPU, memory, and disk. An account-scoped capacity capability returns its
allowed sizes with resource amounts and hourly price. Price carries explicit
currency and units. A typed temporary start refusal retains retry timing.
Plan-size refusal and billing refusal stay separate. Account limits remain
advisory. A create response decides whether a start succeeded.

Shared lifetime validation checks representability and whole positive seconds.
Each provider applies its own maximum. The Boat adapter must accept six-hour
one-shot work on an eligible paid account. Preserve E2B's existing limit and
non-mutating one-shot access. Account trial restrictions remain typed provider
refusals. Access must never extend a one-shot sandbox's original deadline.

Boat uses `https://boat.dev/api/v1` with a Bearer key. Create sends one
`POST /sandboxes` with an `Idempotency-Key` derived from the durable operation.
It sends `noEnv: true`, `snapshots: false`, `monitoring: false`, size, TTL,
correlation labels, and a saved snapshot reference when requested. Recovery
only reads provider state and never repeats a create. Saved snapshots use
their dedicated API. Delete uses repeated sandbox DELETE responses until
confirmed completion or terminal 404. It does not require the organization-
forbidden deletion-operation endpoint.

The Boat capacity implementation filters its published resource and price
catalogue through the account's `machineTypes` from `/limits`. It does not
claim that `/limits` returns resource amounts or prices. Document the source
and date of the published catalogue. Preserve explicit failure for unknown
sizes rather than invent resource or price values.

Implementor call, 4 November 2026: choose Boat's documented exec API when it
can preserve the process and file guarantees. SSH is the alternative if an
API limitation prevents those guarantees. The caller must not need a local
Boat CLI. Keep commands, files, credentials, and runner input out of diagnostics.
Use typed transports, bounded responses, no credentialed redirects, safe
provider route segments, and trusted file operations. Record the final
transport choice and its reason here after checking the implementation.

Implementor call, 4 November 2026: retain `snapshots: false` on every adapter
create. Boat can restore these disposable sandboxes from saved snapshots.
Saved-snapshot capture uses a caller-supplied, separately prepared source with
capture enabled. Boat forbids capture of a source with snapshots disabled.
The snapshot conformance probe accepts an explicit source. Its fake-server
fixture must enforce this provider restriction instead of granting unsupported
capture to disposable creates. This keeps both approved requirements intact.

Implementor call, 4 November 2026: run the milestones in order in this shared
workspace. They edit the same interface. The orchestrator writes the plan,
delegates each milestone to the `non-ui implementor`, checks the result, and
ticks its TODOs. The orchestrator owns the final gate, commits, push, review,
and pull request. The explicit task overrides the shared skill defaults that
delete a completed plan or automatically fix review findings.

## Replaced Ways

- Required `SandboxBackend` methods become capability traits. Consumers:
  E2B, registry types, conformance helpers, mocks, interface tests, E2B tests,
  construction smoke coverage, and examples. Move them in Milestone 2.
- One shared one-hour lifetime ceiling becomes provider-specific validation.
  Consumers: lifetime helpers, E2B validation and metadata, conformance,
  fixtures, and documentation. Move them in Milestone 3.
- The full-backend-only conformance entry point becomes capability probes.
  Keep the existing assertions and resource cleanup guarantees. Move them
  in Milestone 2 and use the supported probes for Boat in Milestone 4.

## Milestone 1: Document The Approved Contracts

Depends on: none

State the approved boundaries before implementation. Keep existing unrelated
file, process, terminal, and screen requirements intact.

- [x] Update the shared contract for capabilities, capacity, start refusals,
      and provider-specific one-shot lifetimes.
- [x] Define the Boat contract from the owner's spike and public API docs.
- [x] Record Juno's separate capability adoption and request migration.
- [x] Check Markdown, local links, and consistency with the approved scope.

Milestone completed after the orchestrator inspected all contract pages,
checked the preservation report, and reran Markdown link and whitespace checks.
Evidence: `.context/sandbox-capabilities-and-boat/reports/milestone-1.md`.

## Milestone 2: Split Backend Capabilities

Depends on: 1

Expose independent traits and capability-specific conformance probes. E2B
continues to support all behavior that it supports before this change.

- [x] Split all 28 methods into object-safe async capability traits and mocks.
- [x] Replace monolithic registry and shared aliases with capability access.
- [x] Move E2B implementations and every in-repository consumer to the split.
- [x] Split conformance dependencies without losing recovery, bounded process,
      identity, or unconditional cleanup assertions.
- [x] Update crate READMEs and examples for the real new public API.
- [x] Run format, focused interface/E2B tests, and the construction smoke test.
- [x] Orchestrator inspects the diff and verifies the milestone TODOs.

Milestone completed after the orchestrator checked the method allocation,
dispatch, registry, cleanup, probes, and test-preservation evidence. The
orchestrator reran focused conformance and construction smoke checks.
Evidence: `.context/sandbox-capabilities-and-boat/reports/milestone-2.md`.

Implementor calls, 4 November 2026: use nine independent traits and explicit
registry accessors. Optional capabilities return `None`. Composition roots
share `Arc<E2bSandboxBackend>` through the specific trait objects they need.
The complete conformance scenario uses an explicit dependency value. Each
tracked resource retains its cleanup capability. These calls preserve all
existing operations without requiring unsupported provider methods.

## Milestone 3: Add Capacity And Start Contracts

Depends on: 2

Let ciproxy select capacity, inspect eligible account sizes, distinguish start
refusals, and request six-hour one-shot work from a capable provider.

- [x] Add typed provider/resource sizing to service and backend creates.
- [x] Add an account capacity trait with resource amounts and hourly prices.
- [x] Add distinct typed temporary start, plan-size, and billing refusals.
- [x] Replace the shared one-hour ceiling with provider-specific validation.
- [x] Preserve E2B's existing size/profile behavior and lifetime limit.
- [x] Update mocks, fixtures, conformance, READMEs, and adoption guidance.
- [x] Prove the changed requirements with focused tests and assertion failures.
- [x] Orchestrator inspects the diff and verifies the milestone TODOs.

Milestone completed after the orchestrator inspected the public types,
provider validation, metadata parsing, and definitive-refusal cleanup. The
orchestrator reran shared and E2B lifetime, sizing, and refusal tests.
Evidence: `.context/sandbox-capabilities-and-boat/reports/milestone-3.md`.

Implementor calls, 4 November 2026: use an optional size-selection enum,
explicit decimal and binary resource units, and integer decimal price amounts.
Account capacity is an optional capability. Temporary, plan, billing, and trial
refusals have distinct typed reasons. Relative and UTC retry times stay
separate and optional. Shared lifetime validation checks `u32` representation.
Each provider owns its maximum. Definitive refusals leave no conformance
recovery or cleanup obligation because no sandbox started.

## Milestone 4: Implement Boat Control And Capacity

Depends on: 3

Build a working Boat adapter for lifecycle, saved snapshots, and account
capacity. Verify the concrete HTTP boundary before adding the trusted runtime.

Orchestrator call, 4 November 2026: split Boat control and runtime into two
milestones. The trusted runtime depends on a tested control transport and fake
server. This keeps each delegated unit functional and reviewable.

- [x] Add the crate, workspace dependencies, configuration, typed transport,
      and a publishable crate README.
- [x] Implement create, read-only recovery, inspect, account inventory, and
      idempotent deletion without the deletion-operation endpoint.
- [x] Preserve correlation, owner, consumer, size, network, and lifetime data.
- [x] Keep absent snapshot selection distinct from every valid saved name.
- [x] Implement saved snapshot create, recovery, inspection, restore, and delete.
- [x] Preserve failed-save identity for cleanup without treating failure as a
      completed snapshot or weakening source reconciliation.
- [x] Implement the account-filtered published size catalogue and exact refusal
      mapping, including retry timing and the trial lifetime refusal.
- [x] Preserve documented organization names in the shared billing header.
- [x] Test lifecycle, secrets isolation, idempotency, recovery, delete, sizing,
      refusals, and snapshots against a local fake Boat server.
- [x] Run lifecycle, one-shot, and snapshot conformance against that fake server.
- [x] Build the new crate and smoke-test its public construction and fake API use.
- [x] Prove new tests fail on behavioral assertions. Repeat new asynchronous
      tests ten times and save counts, timing, and failure evidence in `.context/`.
- [x] Update workspace and crate docs for the verified control API.
- [x] Orchestrator inspects the diff and verifies the milestone TODOs.

Milestone completed after the orchestrator inspected control, identity,
transport, sizing, and failure handling, then reran all 27 Boat tests.
Evidence: `.context/sandbox-capabilities-and-boat/reports/milestone-4.md`.

Implementor calls, 4 November 2026: use versioned exact-identity labels,
disjoint optional-source encodings, and local minimum-resource selection.
Read the exact saved name before capture to prevent replacement. Preserve
failed snapshot identity for cleanup and require Ready for successful capture.
Use a local TLS fixture with added trust roots to retain production validation.

## Milestone 5: Implement Boat Processes And Files

Depends on: 4

Add the process and file capabilities to the tested Boat adapter. Use the
documented exec and attach APIs with trusted framing and protected file chunks.
Keep the existing shared safety guarantees.

- [x] Implement bounded direct-argv collected, streaming, and stateless process
      execution with exact bytes, completion, timing, and cleanup guarantees.
- [x] Keep trusted final-ack identity until stateless overflow cleanup completes.
- [x] Release a dropped stream without a kill when its PID grace finds no child.
- [x] Attempt bounded preparation cleanup after bootstrap errors or cancellation.
- [x] Implement bounded secure regular-file reads and atomic replacement writes.
- [x] Reuse trusted file helper assets through a neutral internal crate when
      both providers need them. Keep provider transports separate and build
      every added crate.
- [x] Test process and file behavior through the local fake Boat server.
- [x] Run the applicable shared process and file conformance probes.
- [x] Test trusted helpers on real temporary files and prove controller/workload
      separation with separate operating-system identities.
- [x] Prove new tests fail on behavioral assertions. Repeat new asynchronous
      tests ten times and save counts, timing, and failure evidence in `.context/`.
- [x] Build and smoke-test the final crate and every added helper crate.
- [x] Smoke-test an independent runner surviving normal launcher completion.
- [x] Update workspace docs and record the verified transport choice above.
- [x] Orchestrator inspects the diff and verifies the milestone TODOs.

Milestone completed after the orchestrator checked file safety, helper trust,
preparation cleanup, asset equality, and all 64 Boat tests. A separate read-only
check verified stream framing, queues, timing, identity, and drop behavior.
Evidence: `.context/sandbox-capabilities-and-boat/reports/milestone-5.md` and
`reports/stream-contract-acceptance.md` in that evidence directory.

Verified transport call, 4 November 2026: use configured-origin HTTPS detached
exec and attach with private manifests, binary framing, and protected chunks.
SSH adds no required capability. Restrict caller work to a separate UID with
no new privileges. Retain release, final-ack, and trusted OS identities for
short commands and cleanup. Share unchanged file assets through
`sandbox-runtime-assets`. Normal launcher completion preserves an independently
detached runner. The real helper smoke proved that behavior and reaped its child.

## Milestone 6: Verify, Commit, Push, Review, And Open The Pull Request

Depends on: 1, 2, 3, 4, 5

Give the owner a tested and reviewed pull request into `main`. Keep findings
for the owner's decision. Do not merge the pull request.

- [x] Run relevant tests, formatting, Clippy, smoke coverage, and file-size checks.
- [x] Run `cargo xtask check` and resolve all compile and check failures.
- [x] Audit removals against `origin/main`, documentation drift, and the full diff.
- [x] Merge mainline documentation updates and rerun the complete gate.
- [x] Run `git add -A`, commit with Conventional Commits, and push this branch.
- [x] Run `cargo xtask review` after the push against the clean pushed branch.
      Save and report every finding. Do not fix its findings automatically.
- [x] Open the pull request into `main` with this plan, decisions, and validation.
- [ ] Check the final commit's CI and report its exact results.
- [ ] Update this plan's status and `plans/README.md` when all work is checked.
      Keep outstanding owner findings explicit. Commit and push bookkeeping.
      Re-run `cargo xtask review` if the reviewed commit changes.

Evidence: `.context/sandbox-capabilities-and-boat/` holds prompts, reports,
public documentation extracts, and verification logs. No provider key is needed.

Mainline integration: preserve the documentation and follow-up records from
`2f35916`. Combine provider guidance in `README.md`. Keep only this requested
plan in `plans/README.md`. No Rust, Python, or Cargo content changes in the merge.
