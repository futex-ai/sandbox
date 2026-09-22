# Egress Allowlist Policy

Add a provider-neutral outbound allowlist policy and implement its E2B wire,
recovery, conformance, and live-proof behavior without weakening deployment
deny rules.

## Milestone 1: Provider-Neutral Policy Contract (Completed)

Define a validated, canonical allowlist value while keeping unsupported future
policy variants rejectable by adapters.

- [x] Add typed IP, CIDR, and domain egress destinations.
- [x] Add bounded allowlist construction and raw-value revalidation.
- [x] Add validation and serialization tests for valid and invalid policies.
- [x] Document domain port limits and IP-based filtering outside ports 80/443.

## Milestone 2: E2B Creation And Recovery (Completed)

Translate both policy variants to exact E2B create bodies and preserve private
and deployment-owned deny rules.

- [x] Reject allow entries that overlap private or profile deny ranges.
- [x] Preserve the existing `Open` create body and encode canonical `allowOut`
      entries for `Allowlist`.
- [x] Correlate allowlist policy identity and return a typed recovery mismatch.
- [x] Add byte-exact body, deny-overlap, and create/recovery regression tests.

## Milestone 3: Conformance And Live Proof (Completed)

Exercise deny-by-default behavior in the shared credential-free harness and in
an explicitly credentialed E2B smoke test.

- [x] Add a shared allowlist sandbox and allowed/disallowed fetch probe.
- [x] Extend fake backends and transports without adding network access to the
      default test suite.
- [x] Add an ignored `live-e2b` proof using the same shell-level behavior.
- [x] Run focused interface, adapter, conformance, and live-feature tests.

## Milestone 4: Documentation (Completed)

Align the public contract, adapter guarantees, and crate entry points with the
implemented policy.

- [x] Update `docs/sandbox-contract.md` and `docs/e2b-adapter.md`.
- [x] Update both crate `README.md` files and relevant live-test commands.
- [x] Review the completed diff and move this plan to completed status.

## Milestone 5: Validation And Delivery (Completed)

Complete the repository-required validation and review workflow.

- [x] Run Rust formatting, focused linting/tests, and relevant smoke checks.
- [x] Run `cargo xtask check` with a 100% passing result.
- [x] Stage every change, commit with a Conventional Commit, and push the
      current branch.
- [x] Run `cargo xtask review` after the push.
- [x] Report every review finding with severity, context, impact, lettered
      options, and a recommendation without automatically changing the code.

## Review Outcome

The post-push review identified three findings retained for maintainer choice:
noncanonical IPv4 spellings can pass domain validation, IPv4-mapped IPv6 rules
can evade IPv4 deny overlap, and the denied-host conformance command can treat
an HTTP error response as a successful denial proof. No finding was changed
automatically; the delivery report includes the required severity, impact,
options, and recommendation for each item.
