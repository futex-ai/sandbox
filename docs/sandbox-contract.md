# Sandbox Contract

This contract defines the boundary between a trusted consumer and a sandbox
provider. Application persistence and orchestration remain with the consumer.

The linked pages are part of this contract:

- [File safety](sandbox-file-safety.md).
- [Processes](sandbox-process-contract.md).
- [Terminals and screens](sandbox-terminal-screen-contract.md).
- [Snapshots and image preparation](sandbox-image-contract.md).
- [Transport and diagnostics](sandbox-transport-safety.md).
- [Capability conformance](sandbox-conformance.md).

## Identity And Ownership

Consumer-owned sandbox, snapshot, terminal, action, and operation IDs are
stable UUIDv7 values. Provider references are opaque strings and must not be
parsed outside an adapter. Every operation carries enough owner and resource
identity for the trusted service to authorize the graph before provider work.

`SandboxConsumer` separates ordinary runtime sandboxes from browser-session
sandboxes. Backend creation requests preserve that class in provider metadata.
Managed inventory returns it when recognized and uses `None` for older
resources that lack the metadata; adapters must not guess. Operations that are
not valid for a class fail with a typed error before dispatch.

## Independent Capabilities

Consumers depend on the capabilities that their work requires. A provider
implements only the capabilities that it supports. No provider must implement
an interface that requires every capability.

| Capability | Required behavior |
| --- | --- |
| Lifecycle | Create, recover an uncertain create, inspect, list managed sandboxes, and destroy idempotently. |
| Resumable lifecycle | Resume and pause sandboxes whose lifetime permits those operations. |
| Saved snapshots | Inventory, create, recover uncertain creation, inspect, and delete saved snapshots. Lifecycle creation can restore a supported snapshot. |
| Image preparation | Stage files, run setup and ordered verification, scrub, and return a measured size without allocating or deleting resources. |
| Processes | Bounded direct-argv execution with collected or incremental stdout and stderr, plus stateless read-only execution. |
| Files | Bounded regular-file reads and atomic replacement writes below a trusted root. |
| Terminals | Create, recover, inspect exact identity, read bounded transcripts, send input, close, and clean restored terminals. |
| Port ingress | Call-local HTTP ingress with an optional redacted credential. |
| Screens | Idempotently ensure the screen stack and acknowledge an exact validated viewport resize. |
| Account capacity | Report eligible provider sizes with resource amounts and hourly prices for the configured account. |

Each supported capability must satisfy all of its shared safety requirements.
The split does not weaken a provider's existing behavior. E2B continues to
support its existing lifecycle, snapshot, image, process, file, terminal,
ingress, and screen behavior. Boat supports the subset defined in its
[adapter contract](boat-adapter.md).

Managed inventory preserves recognized consumer, lifetime, and correlation
values. Missing or malformed optional metadata stays unknown. An adapter must
not infer those values from provider defaults.

## Capacity And Start Results

A create request can select a provider size or request vCPU, memory, and disk
resources. These are alternatives. Resource amounts carry explicit units.
An accepted resource request must not silently receive less capacity.
A provider must preserve its existing profile behavior when no size is selected.
Unsupported sizes or resource requests retain a typed refusal.

Account capacity reports only sizes eligible for the configured account.
Each offer identifies its provider size, vCPU count, memory amount, disk amount,
and hourly price. Prices carry an explicit currency and billing unit.
An adapter must not invent resource amounts or prices for an unknown size.

Account capacity and remaining limits are advisory. They do not reserve a
machine or guarantee that a later create will succeed. The create result
settles whether the provider accepted that start.

A temporary start refusal is distinct from ambiguous delivery. It confirms
that the refused start did not run. It retains the provider's retry delay and
next permitted start time when supplied. Consumers can use that timing without
parsing diagnostic text.

A plan-size refusal remains distinct from a billing refusal. A temporary
capacity or start-window refusal must not hide either refusal. Account trial
restrictions also remain typed provider refusals. A refusal must not look like
a queued start or a successful create. An uncertain mutation remains subject
to recover-only handling.

## Sandbox Lifetimes

Both service and backend create requests carry one of two lifetime policies:

- `IdleAutoPause` is the default for resumable interactive sandboxes.
- `OneShot { max_lifetime }` never pauses or resumes. It runs until explicit
  destruction or the original provider deadline ends the sandbox.

Shared one-shot validation requires a positive, whole-second duration that the
provider request can represent. There is no shared one-hour maximum.
Each adapter validates its own maximum before create or recovery accesses the
provider. A duration beyond an account's allowance retains the provider's
typed refusal when that allowance is determined by the provider.

E2B retains its maximum of 3,600 seconds. Boat accepts six-hour one-shot work
on an eligible paid account. Boat's paid maximum is 30 days. Its two-hour trial
restriction remains a typed provider refusal. The
[E2B](e2b-adapter.md#sandbox-lifetimes) and
[Boat](boat-adapter.md#lifetimes) contracts define their provider behavior.

Adapters preserve the lifetime kind and one-shot maximum in recovery metadata.
Managed inventory returns a lifetime only when that metadata is complete and
valid. Missing, unknown, or malformed metadata remains unknown.

Access to a running one-shot sandbox must use a non-mutating path. Resume,
connect, pause, process execution, and other access must not extend its original
deadline. A stopped one-shot sandbox must not resume. Consumers should destroy
completed work explicitly instead of waiting for the provider deadline.

## Network Policies

`SandboxNetworkPolicy::Open` adds no per-session restriction. It never weakens
the deployment-owned private-network or profile deny rules applied by an
adapter. `SandboxNetworkPolicy::Allowlist` denies ordinary outbound traffic by
default and permits only its typed `EgressDestination` values.

An allowlist accepts no more than 64 caller-supplied entries. Exact IP values
use `IpAddr`; CIDRs carry an address and a family-appropriate prefix and are
canonicalized to their network address. IPv4-mapped IPv6 addresses and CIDRs
contained by the mapped prefix canonicalize to IPv4 before policy identity and
overlap checks. Domain values are lowercase ASCII DNS names no longer than 253
bytes. Each label is `1..=63` bytes, begins and ends with an ASCII letter or
digit, and otherwise contains only letters, digits, or hyphens. A domain may
have one leading `*.` label, which matches subdomains at any depth but not the
apex. Bare wildcards, embedded wildcards, canonical or legacy URL-style IP
literals represented as domains, schemes, ports, paths, leading or trailing
dots, and empty labels are invalid. Construction sorts and
deduplicates canonical entries, but the original list must meet the 64-entry
bound before deduplication. Adapters revalidate values from every construction
or deserialization path before provider dispatch.

Domain matching covers HTTP on port 80 through the `Host` header and TLS on
port 443 through SNI. It does not cover QUIC/HTTP3 or arbitrary ports; those
flows are controlled only by allowed IP and CIDR values. A provider whose
allow rules outrank deny rules must reject any allowed IP or CIDR overlapping a
private or deployment deny range before mutation. Cross-family checks treat
IPv4 as its mapped IPv6 range so broader IPv6 CIDRs cannot bypass an IPv4 deny.
An implicitly allowed DNS resolver must pass the same check.

Destination kinds describe the provider-neutral policy vocabulary, not a
promise that every adapter can enforce every kind. If a provider cannot apply
the complete policy without weakening private or deployment deny rules, its
adapter must return `UnsupportedNetworkPolicy` before any provider request.
The adapter must not silently omit the unsupported destination or partially
apply the policy.

Create recovery receives the exact original policy. A backend must correlate
the policy applied by create, revalidate the recovery value, and return
`SandboxNetworkPolicyMismatch` when the correlated sandbox used a different
policy. It must not silently adopt that sandbox or dispatch a replacement.

## Creation And Recovery

Creation and snapshot methods separate an initial mutation from recovery. The
trusted caller must durably record dispatch intent before the first mutation.
After that call begins, every retry uses the matching recovery method with the
same request; an empty eventual-consistency inventory remains in progress and
must not trigger another create. If delivery cannot be proven, the provider
uses stable correlation data to recover exactly one resource or fails closed.
Sandbox creation must dispatch its one provider mutation directly after local
validation. It must not put a fallible inventory read before that dispatch,
because the caller cannot safely replay a create after invocation starts.
Ambiguous delivery may perform recover-only inventory reads but must never send
a second provider create.
Snapshot inventory requires a nonempty source provider reference and nonempty
correlation value before an authenticated provider request is built. Snapshot
creation requires the same nonempty values before its mutation is sent.

Provider idempotency support supplements this rule. It does not authorize the
adapter to replay an uncertain create. Recovery must use the original owner,
consumer, correlation, size, lifetime, network policy, and snapshot selection.
It must never adopt a resource whose recorded request identity conflicts.

## Common Failure Safety

Port zero, empty required text, oversized values, unknown profiles, invalid or
unsupported network policies, and allowlist conflicts with deployment denies
fail before provider dispatch. Bounds must be checked before provider access.
Absolute deadlines use checked arithmetic so caller durations cannot panic.

Capability-specific requirements remain mandatory for files, processes,
terminals, screens, snapshots, and image preparation. Provider credentials,
route validation, exact acknowledgments, and diagnostic privacy follow the
[transport and diagnostics contract](sandbox-transport-safety.md).

Adapters run the applicable [conformance probes](sandbox-conformance.md).
A provider needs no unsupported capability to run a supported probe.
