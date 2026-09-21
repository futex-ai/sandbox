# E2B Adapter Guarantees

`sandbox-e2b` implements the shared contract with E2B's control API and envd
process API. Default builds, tests, and smoke checks construct clients but make
no external request.

## Configuration

`E2bAdapterConfig::new` requires:

- a stable backend ID;
- an HTTPS root API origin;
- a non-empty API key;
- a non-empty logical profile map with template IDs and at least one denied
  IP or CIDR per profile; and
- a nonzero idle timeout.

Profile names use the shared lowercase reference-safe format and limits.
Denied destinations are parsed as IP addresses, canonicalized, sorted, and
deduplicated. Validated fields are externally immutable; callers can inspect
non-secret values through read-only accessors and can replace runtime
conventions only with an already validated value.

`E2bRuntimeConventions` controls the metadata prefix, terminal process-tag
prefix, absolute screen-helper path, and exact helper and agent process names
stopped before an image snapshot. Neutral defaults are suitable for new
deployments. Existing deployments should explicitly supply their current
values so recovery and snapshot cleanup find the right resources. Unsafe
prefixes, paths, shell characters, whitespace, and process names longer than
Linux's 15-byte task-name limit are rejected.

## Transport And Reconciliation

Control and process transports are traits and can be injected. Control calls
use bounded connect, read, and total timeouts. Mutating timeouts remain
delivery-ambiguous; safe reads and idempotent deletes become retryable provider
unavailability. Control, unary process, and file-response bodies are consumed
as chunks and stop as soon as their cumulative byte limit is exceeded. Clients
that carry an API or envd access token never follow HTTP redirects.
Definitive non-success response headers are mapped without waiting for their
unused bodies, so a rejected mutation cannot become delivery-ambiguous merely
because that error body stalls.

Sandbox creation filters on exact configured metadata. Snapshot recovery walks
bounded cursor pagination and adopts exactly one new correlated snapshot.
Repeated cursors, excessive pages, identity mismatches, and multiple candidates
fail closed. If an accepted create response cannot be decoded or omits the
identity or credentials needed to identify the created resource, the result
remains delivery-ambiguous and enters recovery.

Envd routing is derived from adapter configuration, not a response-provided
host. The complete HTTPS URL must parse to the exact configured envd hostname
before the access-token header is added. Access tokens and private-traffic
credentials stay inside call-local types and are redacted from debug output.
An unconfigured logical profile returns the handled provider-neutral
`UnknownProfile` error before any provider request.

## Files, Processes, Terminals, And Screens

Regular-file reads use a descriptor-relative helper with non-following opens
and `fstat` on the opened leaf. Replacement writes stage the bounded payload,
then traverse the trusted root through non-following directory descriptors and
atomically replace the leaf. Any failed upload or replacement invokes a bounded
descriptor-relative cleanup helper for both the upload staging path and the
destination temporary name. It repeats destination cleanup after the
process-kill allowance so a late, unconfirmed writer cannot recreate the
temporary name after the first pass. Process execution is direct-argv and keeps
stdout, stderr, deadlines, and overflow outcomes separate. A one-shot process
whose collection times out, overflows, or fails decoding is killed with a
bounded cleanup call once its PID has been observed; persistent terminal
connections are left running intentionally.

Terminal recovery lists processes by the configured stable tag and never
starts a replacement when recovery finds no match. Inspection and output reads
bind the stored PID to that exact tag. Input and close requests select the tag
inside the provider operation itself, so a process that reuses the stored PID
cannot receive input or be killed. Durable log reads share one absolute
provider deadline and coherent cursor/size reporting. Transcript writers use
the request's exact byte limit rather than a rounded filesystem block limit.
Oversized replacement writes fail before acquiring mutating sandbox access.

Successful image realization returns the completed snapshot, measured size,
and source-sandbox cleanup reference without destroying the source first. The
trusted caller must durably store that completion and then call the backend's
idempotent sandbox destroy operation. This keeps transient cleanup failures
from replaying credential-free setup and verification scripts.

Screen ensure and capability discovery invoke the configured helper with
bounded streams. Resize accepts only an exact versioned acknowledgment and
requires a normal helper exit before reporting success. Port zero is rejected
before connecting to a sandbox. A custom screen-capable template is built and
published by the separate template release project;
`E2B_SCREEN_TEMPLATE_ID` selects one only for the ignored live smoke test.

## Live Tests

The `live-e2b` feature only compiles credentialed tests. Every live test is also
marked ignored, so `cargo test --workspace --all-features` remains offline.
Running an ignored test requires an explicit API key, may incur provider cost,
and executes cleanup for tracked terminals, sandboxes, and snapshots.
