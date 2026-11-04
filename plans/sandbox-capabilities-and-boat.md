# Sandbox Capabilities And Boat

Status: Active. Branch: `conductor/ciproxy-mvp-milestone-12-sandbox-capabilities-and-boat`.
No pull request exists yet. This plan stays in the pull request at the owner's request.

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

- [ ] Update the shared contract for capabilities, capacity, start refusals,
      and provider-specific one-shot lifetimes.
- [ ] Define the Boat contract from the owner's spike and public API docs.
- [ ] Record Juno's separate capability adoption and request migration.
- [ ] Check Markdown, local links, and consistency with the approved scope.

## Milestone 2: Split Backend Capabilities

Depends on: 1

Expose independent traits and capability-specific conformance probes. E2B
continues to support all behavior that it supports before this change.

- [ ] Split all 28 methods into object-safe async capability traits and mocks.
- [ ] Replace monolithic registry and shared aliases with capability access.
- [ ] Move E2B implementations and every in-repository consumer to the split.
- [ ] Split conformance dependencies without losing recovery, bounded process,
      identity, or unconditional cleanup assertions.
- [ ] Update crate READMEs and examples for the real new public API.
- [ ] Run format, focused interface/E2B tests, and the construction smoke test.
- [ ] Orchestrator inspects the diff and verifies the milestone TODOs.

## Milestone 3: Add Capacity And Start Contracts

Depends on: 2

Let ciproxy select capacity, inspect eligible account sizes, distinguish start
refusals, and request six-hour one-shot work from a capable provider.

- [ ] Add typed provider/resource sizing to service and backend creates.
- [ ] Add an account capacity trait with resource amounts and hourly prices.
- [ ] Add distinct typed temporary start, plan-size, and billing refusals.
- [ ] Replace the shared one-hour ceiling with provider-specific validation.
- [ ] Preserve E2B's existing size/profile behavior and lifetime limit.
- [ ] Update mocks, fixtures, conformance, READMEs, and adoption guidance.
- [ ] Prove the changed requirements with focused tests and assertion failures.
- [ ] Orchestrator inspects the diff and verifies the milestone TODOs.

## Milestone 4: Implement sandbox-boat

Depends on: 3

Build a real Boat adapter with fake-server tests. It implements only the
supported lifecycle, snapshot, process, file, and account capacity capabilities.

- [ ] Add the crate, workspace dependencies, configuration, typed transport,
      and a publishable crate README.
- [ ] Implement create, read-only recovery, inspect, account inventory, and
      idempotent deletion without the deletion-operation endpoint.
- [ ] Preserve correlation, owner, consumer, size, network, and lifetime data.
- [ ] Implement saved snapshot create, recovery, inspection, restore, and delete.
- [ ] Implement documented process transport and bounded secure file transfer.
- [ ] Implement the account-filtered published size catalogue and exact refusal
      mapping, including retry timing and the trial lifetime refusal.
- [ ] Test lifecycle, secrets isolation, idempotency, recovery, delete, sizing,
      refusals, processes, files, and snapshots against a local fake Boat server.
- [ ] Run the applicable shared conformance probes against that fake server.
- [ ] Build the new crate and smoke-test its public construction and fake API use.
- [ ] Prove new tests fail on behavioral assertions. Repeat new asynchronous
      tests ten times and save counts, timing, and failure evidence in `.context/`.
- [ ] Update workspace docs and record the verified transport choice above.
- [ ] Orchestrator inspects the diff and verifies the milestone TODOs.

## Milestone 5: Verify, Commit, Push, Review, And Open The Pull Request

Depends on: 1, 2, 3, 4

Give the owner a tested and reviewed pull request into `main`. Keep findings
for the owner's decision. Do not merge the pull request.

- [ ] Run relevant tests, formatting, Clippy, smoke coverage, and file-size checks.
- [ ] Run `cargo xtask check` and resolve all compile and check failures.
- [ ] Audit removals against `origin/main`, documentation drift, and the full diff.
- [ ] Run `git add -A`, commit with Conventional Commits, and push this branch.
- [ ] Run `cargo xtask review` after the push against the clean pushed branch.
      Save and report every finding. Do not fix its findings automatically.
- [ ] Open the pull request into `main` with this plan, decisions, and validation.
- [ ] Check the final commit's CI and report its exact results.
- [ ] Update this plan's status and `plans/README.md` when all work is checked.
      Keep outstanding owner findings explicit. Commit and push bookkeeping.
      Re-run `cargo xtask review` if the reviewed commit changes.

Evidence: `.context/sandbox-capabilities-and-boat/` holds prompts, reports,
public documentation extracts, and verification logs. No provider key is needed.
