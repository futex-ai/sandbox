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
deduplicated.

`E2bRuntimeConventions` controls the metadata prefix, terminal process-tag
prefix, and absolute screen-helper path. Neutral defaults are suitable for new
deployments. Existing deployments should explicitly supply their current
values so recovery can find already-created resources. Unsafe prefixes,
relative paths, parent-directory components, whitespace, and oversized values
are rejected.

## Transport And Reconciliation

Control and process transports are traits and can be injected. Control calls
use bounded connect, read, and total timeouts. Mutating timeouts remain
delivery-ambiguous; safe reads and idempotent deletes become retryable provider
unavailability.

Sandbox creation filters on exact configured metadata. Snapshot recovery walks
bounded cursor pagination and adopts exactly one new correlated snapshot.
Repeated cursors, excessive pages, identity mismatches, and multiple candidates
fail closed.

Envd routing is derived from adapter configuration, not a response-provided
host. Access tokens and private-traffic credentials stay inside call-local
types and are redacted from debug output.

## Files, Processes, Terminals, And Screens

Regular-file reads use a descriptor-relative helper with non-following opens
and `fstat` on the opened leaf. Process execution is direct-argv and keeps
stdout, stderr, deadlines, and overflow outcomes separate.

Terminal recovery lists processes by the configured stable tag and never
starts a replacement when recovery finds no match. Subsequent operations bind
the stored PID to that exact tag, protecting against PID reuse. Durable log
reads share one absolute provider deadline and coherent cursor/size reporting.

Screen ensure and capability discovery invoke the configured helper with
bounded streams. Resize accepts only an exact versioned acknowledgment and
reserves time to confirm remote process termination. A custom screen-capable
template is built and published by the separate template release project;
`E2B_SCREEN_TEMPLATE_ID` selects one only for the ignored live smoke test.

## Live Tests

The `live-e2b` feature only compiles credentialed tests. Every live test is also
marked ignored, so `cargo test --workspace --all-features` remains offline.
Running an ignored test requires an explicit API key, may incur provider cost,
and executes cleanup for tracked terminals, sandboxes, and snapshots.
