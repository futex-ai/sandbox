# sandbox-e2b

`sandbox-e2b` is the first concrete implementation of `sandbox-interface`.
Only composition roots should depend on it; application services should keep
using the provider-neutral interface.

## Responsibilities

- Map E2B control-plane lifecycle and snapshot APIs into shared result types.
- Map envd process, PTY, file, terminal-log, and screen operations behind
  injected traits.
- Keep provider IDs, access tokens, traffic credentials, payloads, and errors
  inside the adapter boundary.
- Reconcile ambiguous creation and snapshot delivery without allocating
  duplicate resources.
- Encode deny-by-default IP/CIDR allowlists and reject unsafe destination kinds
  without weakening deployment deny ranges.
- Enforce bounded IO, exact provider identity checks, and private port ingress.
- Keep all credentialed provider tests feature-gated and ignored by default.

## What This Crate Does

`E2bSandboxBackend` implements `SandboxBackend`. Production construction builds
an HTTPS control client and envd process transport; `with_transports` accepts
trait-backed test or alternate transports. Construction itself sends no E2B
request.

`E2bAdapterConfig` validates the API origin, API key, idle timeout, logical
profiles, template IDs, and denied IP/CIDR destinations, then keeps those
values externally immutable. The public concrete control-client constructor
independently requires an HTTPS root origin, a nonempty canonical API key, a
valid sandbox routing domain, and a nonzero idle timeout before it creates a
credentialed transport. Its public create request carries typed allow
destinations, and the concrete client independently canonicalizes IP/CIDR
rules, rejects domains, enforces the 64-entry bound, checks every private and
deployment deny overlap, and rejects an allowlist paired with ordinary internet
access before transport. Its neutral runtime conventions use the `sandbox`
metadata prefix, `sandbox-terminal-` process-tag prefix,
`/usr/local/bin/sandbox-screen` helper, and neutral image-cleanup process names.
The default workload account is the non-root `user`; deployments can select a
different validated non-root account when their compatible templates require
one. Envd requests explicitly authenticate that account instead of inheriting
an unknown template default.
Deployments that must adopt existing resources can supply an
`E2bRuntimeConventions` value; prefixes, the absolute helper path, and exact
cleanup process names are validated before use.

An Open sandbox keeps the profile's existing public-egress setting and omits
`allowOut`. An allowlisted sandbox always disables ordinary internet access
and sends canonical IP/CIDR destinations through E2B's `network.allowOut`,
while the same built-in private and profile deny ranges remain in `denyOut`.
Because E2B gives allow rules precedence, overlapping allowed IP/CIDR ranges
are rejected before control dispatch, including overlap through IPv4-mapped
IPv6 forms. Representable mapped values canonicalize to IPv4.

E2B domain rules trust the sandbox-controlled HTTP `Host` header or TLS SNI,
and E2B allow rules outrank IP denies. The adapter therefore rejects the
complete policy with `UnsupportedNetworkPolicy` whenever it contains a domain,
before any E2B request. A direct request through the public concrete control
client repeats the safety checks and returns `E2bAdapterError::InvalidRequest`
before transport. Use an IP/CIDR rule, another adapter that jointly verifies
hostnames and destination IPs, or a trusted enforcing proxy. Allowlist policy
identity is hashed into provider metadata so recovery returns a typed mismatch
instead of adopting a sandbox created with different egress access. Open bodies
remain unchanged.

Sandbox creates dispatch directly after local validation, without a provider
inventory preflight, and carry exact metadata including the typed consumer and
lifetime. Ambiguous delivery uses only metadata-filtered recovery reads and
never sends a second create. Allowlist policy identity is verified separately
on the returned row. Idle-auto-pause creation preserves the existing
resumable request body. A one-shot create disables pause and resume and uses
its validated 1-to-3600-second maximum as E2B's destruction timeout. Create and
recovery reject invalid lifetimes before provider access. A sandbox ID must be
a lowercase DNS-label fragment that fits every envd hostname. An accepted
create with an unusable ID remains delivery-ambiguous; managed inventory
rejects the same ID as provider unavailability, maps recognized consumer and
lifetime metadata, and leaves missing or malformed values absent for older
resources. Before connect or
pause, the concrete client reads that metadata. Running one-shot envd access
uses the non-mutating detail response; paused one-shot sandboxes are not resumed
and their original timeout is never extended. Because detail does not return a
private-traffic token, one-shot port ingress fails safely. Create and recover
validate the same profile and network-policy rules before any control request.
Snapshot creation and recovery require a nonempty source sandbox and
correlation name before an authenticated request. Bounded, cursor-safe snapshot
inventory rejects an empty or dot-segment identity as provider unavailability;
an accepted snapshot
create with such an unusable identity remains delivery-ambiguous. Snapshot
inspection also rejects a response whose ID differs from the requested ID.
Snapshot capture keeps its source paused while recovery finds no new snapshot
or more than one candidate. The adapter reconnects the source only after create
returns one completed snapshot or recovery proves exactly one new candidate.
Terminal identities combine an E2B PID with the consumer terminal ID; start
and inventory responses reject PID zero before exposing a process. Reads
verify both identity values,
while input and close operations use envd's atomic tag selector so PID reuse
cannot retarget them. Before the login shell can run, the trusted supervisor
initializes its private storage, fsyncs a root-owned `0600` versioned identity
record containing the PID, terminal ID, operation ID, and exact tag under a
temporary name, then atomically publishes the final name without replacement.
Recovery and inspection use it to return the same provider reference as
`Exited` after an immediate shell exit. Live terminals without a record remain
compatible; exited legacy terminals and unknown record versions fail closed.
Inspection and output validate every present record, including for a live
terminal, before using selector-only fallback for a record-free legacy
terminal. This record-aware resolution means unrelated PID reuse cannot hide
an exited terminal's retained transcript while same-tag conflicts fail closed.
Only typed file absence enables legacy fallback; provider terminal absence
propagates. Output reads carry an earlier absolute completion deadline into the
identity helper; the process transport derives its execution cutoff by
reserving the full termination window and return time. Input rejects exited
terminals. Explicit close retains the record so final output remains readable,
and restored cleanup removes all terminal identity state. File
reads use one descriptor-relative, non-following helper; writes stage their
payload, bind it to the caller-computed SHA-256
digest, perform one
descriptor-relative atomic replacement below the trusted root, and use an
atomic digest-bearing commit-or-revoke marker to reconcile an uncertain writer
without relying on a timing delay. The writer and reconciler run as the trusted
account and keep markers below a private root-owned directory that workload
processes cannot traverse or modify. Both paths verify the requested bytes
before replacement. The writer creates a root-owned `0700` directory beside
the destination so the prepared inode stays on the same filesystem, then keeps
that directory descriptor open and renames its private payload through the
descriptor. Renaming or replacing the private directory's visible name cannot
substitute workload bytes. The writer then assigns the replacement to the
configured workload account. When reconciliation finds the exact target after
a crash, it resolves the validated non-root workload account, preserves the
target's current mode, and syncs the file and containing directory before
reporting success.
An unconfirmed revocation returns a fencing error instead of pretending the
write safely failed. A writer removes its commit marker after the replacement
and containing directory are durable. Cleanup also removes and syncs a resolved
marker after an upload failure or definitive writer rejection, because those
paths prove that no writer remains. An uncertain writer retains a revocation or
commit fence for reconciliation, including when cleanup can prove the target
was committed.
Malformed or oversized trusted roots and relative paths fail before a file
operation acquires provider access. Oversized writes fail before sandbox
connection. Every trusted Python helper uses isolated module lookup with site
initialization disabled, so sandbox working-directory modules, `PYTHONPATH`,
and user startup customization cannot run before file checks, cleanup, or
terminal transcript setup. Terminal transcript writers enforce accepted byte
limits exactly rather than rounding to filesystem blocks. Create and recovery
reject limits above the shared 256 MiB regular-file ceiling before connecting
to the sandbox, which keeps every completed transcript readable. A trusted
supervisor clips recorder chunks to the remaining limit, keeps draining
overflow so the interactive terminal stays usable, and waits for the recorder
before exiting.
One-shot processes are killed when collection or streaming terminates without
a decoded process end after learning their PID, including consumer drop. A
decoded but undelivered start remains eligible for a bounded kill; if the
consumer drops after start may have been sent but before a PID is decoded,
the worker retains the open and reader for at most three seconds (and never
past the absolute deadline), even after ordinary idle expiry, to learn the
PID. Without one it releases the stream without killing; a decoded end
prevents the kill. HTTP bodies, process output, and terminal output are bounded
while streaming.
Connect frame headers are validated before the rest
of an HTTP chunk is retained, so an oversized declared frame cannot force an
unbounded intermediate buffer. Both combined and split-stream collectors
keep reading after a process end and decode the required Connect trailer and
HTTP EOF through one path: a missing trailer or malformed JSON fails closed, an error
object is retryable provider unavailability, and a present trailer with a
missing or null error field permits a clean close once HTTP EOF follows.
Collectors keep polling after the trailer and reject later bytes, response
errors, or timeout before EOF, including across HTTP chunks.
Incremental execution emits ordered start, stdout, stderr, exit, and final
outcome events. Exit events retain envd's normal-exit flag, and a successful
trailer and HTTP EOF are required for `Completed`; that outcome does not by
itself mean the command succeeded. Timer expiry after exit but before HTTP EOF
is a transport failure, including while exit-event delivery is blocked. Only
stdout or stderr bytes reset the idle timer, measured by the independent
provider reader when the HTTP fragment arrives, even if event delivery stalls.
Silence before the start event produces `IdleTimeout`, whereas EOF without a
trailer produces `TransportFailure`.
The reader bounds decoded-event staging to 32 slots and reports
`ConsumerBackpressure` if staging fills; it stops the process without waiting
for a slow consumer. The terminal outcome and any final bounded overflow
prefix are published through a slot independent from the bounded data queue.
Queued data drains in order before that optional prefix, outcome, and EOF,
while cleanup starts without waiting for consumer capacity.
Direct process, streaming, and stateless read-only requests are
validated before the adapter acquires sandbox access: commands must be
non-empty, combined argv is capped at 128 KiB, and each direct stream or
combined stateless output cap is at most 64 MiB. Caller-controlled process,
read-only execution, and regular-file durations cannot exceed 300 seconds.
Terminal output waits cannot exceed 30 seconds. Every bound is checked before
provider access, and absolute Tokio deadlines use checked arithmetic. Failed
image-command diagnostics contain only exit and capture metadata. Captured
text never enters the handled error, so credentials and provider identities
need no output matching or normalization. Malformed process data with zero or multiple output channels is rejected instead of
silently losing bytes.
Incremental streams instead accept up to 3,600 seconds and require a nonzero
idle timeout no greater than the deadline. Their envd HTTP timeout is the
remaining budget plus a 10-second transport allowance; the absolute budget
starts before control connection, and the idle timer starts before envd access.
For resumable sandboxes the connection timeout is at least the stream deadline;
one-shot sandboxes retain their original lifetime. Expiry during connection
returns `DeadlineExpired` without starting a process.
Direct process starts also forward an optional validated absolute working
directory and environment map. Cwd is capped at 4,096 UTF-8 bytes and rejects
NUL or control characters. Environment names follow
`[A-Za-z_][A-Za-z0-9_]*`, values reject NUL, and requests allow at most 256
entries and 64 KiB across names and values. `PATH`, `HOME`, `LD_*`, and
`DYLD_*` are rejected as template-owned. Execution-request and captured-output `Debug` contain
only selected metadata; streaming commands show argument counts, limits, and
timeouts, while streamed output events show byte counts. They omit command text, paths, environment names or
values, or captured contents. Validation errors never echo a rejected name or
value. Process and terminal bytes remain unmasked in returned results and
saved transcripts; only their automatic diagnostic representations omit
content. Tracing uses static events and selected metadata. Read-only execution
keeps its existing explicit cwd and passes no environment entries; PTY startup keeps its
fixed locale and terminal map.
The shared conformance probe verifies the selected cwd and environment on
stdout while independently asserting a deterministic stderr token.
Credentialed clients, including opt-in live ingress probes, do not follow
redirects, and envd URLs are validated before call-local credentials are
attached. Process, read-only, and private-port hosts use the adapter's validated
configured domain even when an injected control transport returns a different
domain. Empty provider IDs and IDs equal to `.` or `..` are rejected before
an API-key-authenticated control request can be built. Definitive rejection
headers are mapped without waiting for an unused response body. DNS,
connection, timeout, and response-stream failures remain typed as provider
unavailability; failed mutating delivery remains ambiguous. A successful safe
response with malformed JSON is also retryable provider unavailability. A
terminal-input response must decode E2B's exact empty JSON acknowledgment;
unknown fields or malformed output after that accepted mutation keep the
delivery outcome ambiguous. Sandbox create and ordinary connect responses must
contain nonblank envd and private-traffic tokens. Missing or blank credentials
keep an accepted create delivery-ambiguous and make ordinary connect retryable.
One-shot read access requires only the nonblank envd token returned by sandbox
detail and never attempts an unauthenticated envd request.

Image construction is split across the interface's durable phases. E2B
preparation accepts an already persisted source and never creates, snapshots,
or destroys a provider resource. Every staged input path and size is checked
before the adapter connects or writes the first file. Consumers dispatch
source and snapshot creates once, use only their recovery methods after each
dispatch starts, and persist preparation's measured size before snapshot
dispatch. Empty recovery inventory stays in progress and keeps the source
paused instead of reconnecting it, replaying preparation, or allocating another
resource. Multiple candidates likewise keep the source paused for
reconciliation. Before measuring a prepared source, configured image processes
must exit after bounded TERM/KILL
escalation. Size traversal or I/O failure returns `ImageSizeUnavailable`
instead of accepting a partial total. The size-measurement command authenticates
as root so it can traverse private adapter storage; setup and verification
remain on the configured workload account. Restored-sandbox cleanup and image
preparation apply the same bounded escalation to inherited drive helpers and
fail unless those helpers are confirmed gone. Home-directory cache cleanup
uses non-following directory descriptors; a symlinked parent fails scrub and a
child symlink is removed without traversing its target. Terminal log-directory
creation and restored cleanup traverse from directory descriptors with non-following
opens; an intermediate symlink makes the operation fail without touching its
target. Image setup, verification, scrub, and size measurement use non-login
shells, so a staged or setup-created profile cannot skip a later safety phase
or forge its result. The terminal transcript wrapper runs a supervisor and
recorder as the trusted account and opens its log below
private root-owned storage through non-following directory descriptors. The
supervisor keeps the only log descriptor and bounds data received over a
private recorder pipe. The recorder's shell child closes every inherited
private descriptor, drops to the configured workload account, and only then
starts the interactive login shell. Reads use the trusted account, require the
exact terminal-derived log name, and use the same descriptor-relative
regular-file helper. Terminal discovery, input, and shutdown also use the
trusted supervisor account, so the workload cannot replace or forge stored
output and provider-side user scoping cannot hide the terminal from lifecycle
operations.

Screen ensure and resize commands use the configured template helper. Resize
accepts only the exact version, width, and height response with no extra fields,
keeps one absolute deadline, reserves cleanup time, and reports an unconfirmed
termination separately so callers do not release a possibly active session.
Compatible custom template publication belongs to the external template
release project, not this crate.

## Quick Start

```rust
use std::collections::HashMap;
use sandbox_e2b::{E2bAdapterConfig, E2bProfile, E2bSandboxBackend};

let config = E2bAdapterConfig::new(
    "e2b",
    "https://api.e2b.app",
    "placeholder-api-key",
    HashMap::from([("general".to_owned(), E2bProfile {
        template: "base".to_owned(),
        allow_public_egress: true,
        denied_destinations: vec!["169.254.169.254/32".to_owned()],
    })]),
    600,
)?;
let _backend = E2bSandboxBackend::new(config)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Development

Credential-free checks do not contact E2B:

```bash
cargo test -p sandbox-e2b
cargo test -p sandbox-e2b --test screen_resize_deadline
cargo clippy -p sandbox-e2b --all-targets --all-features -- -D warnings
```

Live tests are opt-in, ignored, and billable:

```bash
E2B_API_KEY=... cargo test -p sandbox-e2b \
  --features live-e2b --test live_e2b -- --ignored

E2B_API_KEY=... cargo test -p sandbox-e2b \
  --features live-e2b --test live_e2b \
  live_e2b_one_shot_create_and_destroy -- --ignored

E2B_API_KEY=... E2B_SCREEN_TEMPLATE_ID=... \
  cargo test -p sandbox-e2b --features live-e2b --test live_e2b \
  live_e2b_private_screen_bridges -- --ignored
```

Every live sandbox create request is stored before dispatch. One shared helper
uses bounded, one-second recover-only polling after an uncertain result and
registers the provider handle before restored-terminal cleanup or other later
work can fail. Final cleanup retries unresolved sandbox recovery before
destroying tracked resources. The lifecycle test applies the same ownership to
snapshot requests and retries recovery when the initial flow did not obtain a
deletable snapshot handle.

### Key Code

- `src/config.rs` — validated profiles and adapter configuration.
- `src/runtime_conventions.rs` — validated deployment and cleanup names.
- `src/backend/configured.rs` — backend construction and trait dispatch.
- `src/backend/image_realization.rs` — one-shot caller-owned image preparation.
- `src/backend/image_cache_cleanup.rs` — non-following cache removal.
- `src/backend/terminal_storage.rs` — non-following terminal path helpers.
- `src/backend/terminal_record.rs` — strict durable terminal identity records.
- `src/control/` — E2B control API boundary.
- `src/network.rs` — allowlist translation, deny overlap, and recovery identity.
- `src/process/helper_run.rs` — absolute helper execution and cleanup deadlines.
- `src/process/` — envd Connect framing and operations.
- `src/process/stream_run.rs` — incremental events, timers, and cleanup.
- `src/backend/sandboxes.rs` — metadata correlation and lifecycle mapping.
- `src/backend/sandbox_metadata.rs` — lifetime and correlation metadata.
- `src/backend/screen_resize.rs` — deadline and termination guarantees.

### Related Docs

- [Sandbox contract](../../docs/sandbox-contract.md)
- [E2B adapter guarantees](../../docs/e2b-adapter.md)
- [Process data and diagnostics](../../docs/process-diagnostics.md)
- [E2B snapshot documentation](https://e2b.dev/docs/sandbox/snapshots)
