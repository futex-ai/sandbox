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

`ReqwestE2bControlApi::new` applies the same safety checks at its public
boundary: it rejects a non-HTTPS or non-root API origin, an empty or padded API
key, an invalid sandbox routing domain, and a zero idle timeout before building
the authenticated transport.

`E2bRuntimeConventions` controls the metadata prefix, terminal process-tag
prefix, absolute screen-helper path, exact helper and agent process names
stopped before an image snapshot, and the non-root workload account. Neutral
defaults use the `user` account. Existing deployments should explicitly supply
their current values so recovery and snapshot cleanup find the right resources
and commands use the account present in their templates. Root, unsafe account
names, unsafe prefixes or paths, shell characters, whitespace, and process
names longer than Linux's 15-byte task-name limit are rejected.

## Sandbox Lifetimes

E2B creation maps `SandboxLifetime` without changing the existing interactive
payload:

- `IdleAutoPause` sends `autoPause: true`, `autoPauseMemory: true`,
  `autoResume: { enabled: false }`, and the configured idle timeout.
- `OneShot { max_lifetime }` sends `autoPause: false`,
  `autoPauseMemory: false`, `autoResume: { enabled: false }`, and that validated
  whole-second maximum as `timeout`.

The one-shot maximum is 3600 seconds, inclusive. Create and recover apply the
same validation before listing or mutating provider resources. Exact recovery
metadata includes `lifetime`; one-shot metadata also includes
`one_shot_max_lifetime_seconds`, under the configured metadata prefix. Managed
inventory returns a lifetime only when those values form a complete valid
policy. Older, missing, unknown, or malformed metadata stays unknown.

Before E2B connect or pause, the concrete control client reads sandbox detail.
An idle-auto-pause or legacy resource keeps the existing POST behavior. A
running one-shot resource reacquires envd access from that GET only, provided
automatic resume is disabled; it never sends connect, resume, pause, or timeout
updates. A paused one-shot resource is unavailable rather than resumed. E2B
does not return its private-traffic token from sandbox detail, so private-port
ingress also returns backend unavailability instead of extending a one-shot
deadline. The consumer remains responsible for explicit destruction after
work, with E2B's original timeout as the final cleanup bound.

## Transport And Reconciliation

Control and process transports are traits and can be injected. Control calls
use bounded connect, read, and total timeouts. Mutating timeouts remain
delivery-ambiguous; safe reads and idempotent deletes become retryable provider
unavailability. Control, unary process, and file-response bodies are consumed
as chunks and stop as soon as their cumulative byte limit is exceeded. Clients
that carry an API or envd access token never follow HTTP redirects. The
streaming Connect decoder copies only one header and its declared bounded
payload at a time; an oversized declaration is rejected before the rest of the
HTTP chunk is copied into decoder state. Combined and split-stream collectors
share one end-stream decoder. After either collector observes a process end, it
keeps reading until it consumes the final trailer. A missing trailer, malformed
trailer JSON, or non-null error object fails collection; a present trailer with
a missing or null error field is successful completion.
Definitive non-success response headers are mapped without waiting for their
unused bodies, so a rejected mutation cannot become delivery-ambiguous merely
because that error body stalls.
Control and envd DNS, connection, timeout, request, decode, body, and
response-stream failures map to retryable provider unavailability for safe
reads. This includes a successful HTTP response whose JSON does not match the
safe operation's response type. A failure that can occur after a process start,
terminal input, or upload was delivered remains delivery-ambiguous until its
operation-specific recovery fence resolves the outcome. `SendInput` also
decodes E2B's typed empty response, so a successful HTTP status with malformed
JSON, a non-object response, or an object with any field remains
delivery-ambiguous instead of being treated as an acknowledgment.
Empty opaque provider IDs and IDs equal to `.` or `..` are rejected before
route construction, so URL normalization cannot move an API-key-authenticated
call outside its intended sandbox or snapshot endpoint. Returned sandbox IDs
must additionally be lowercase DNS-label fragments no longer than 57 bytes so
the port prefix and ID fit E2B's 63-byte envd label. Snapshot creation and
inventory retain the broader opaque route-segment format. An unusable ID from
an accepted create remains delivery-ambiguous; an unusable inventory row is
retryable provider unavailability.

Sandbox creation filters on exact configured metadata, including the stable
runtime-or-browser consumer and lifetime values. Managed inventory returns
recognized consumer and lifetime values, rejects an unusable sandbox identity,
and leaves missing or malformed metadata absent. Snapshot recovery
walks bounded cursor pagination and adopts exactly one new correlated snapshot.
The source stays paused when recovery sees no new snapshot or more than one
candidate; only a synchronous completed create or exactly one recovered
candidate triggers reconnect.
Snapshot creation and inventory reject an empty source sandbox or correlation
name before issuing an authenticated request.
Repeated cursors, excessive pages, identity mismatches, and multiple candidates
fail closed. Snapshot inspection checks that an injected control transport
returned the exact requested provider ID. A malformed snapshot identity in
inventory is retryable provider unavailability. If an accepted create response
cannot be decoded or returns an unusable identity or credential, the result
remains delivery-ambiguous and enters recovery.

Envd routing is derived from adapter configuration, not a response-provided
host. Process, read-only, and private-port routing ignore the domain carried by
a control result, including results from an injected transport, and use the
validated configured sandbox domain. The complete HTTPS URL must parse to the
exact configured envd hostname before the access-token header is added. Access
tokens and private-traffic credentials stay inside call-local types and are
redacted from debug output.
Sandbox create and ordinary connect responses require nonblank envd and
private-traffic credentials. Missing or blank credentials keep an accepted
create delivery-ambiguous and make ordinary connect retryable provider
unavailability. One-shot read access requires only a nonblank envd token from
an already running sandbox with automatic resume disabled; it deliberately has
no traffic token. A missing or blank envd token maps to retryable provider
unavailability before any envd request is attempted.
Process and file requests explicitly authenticate the configured workload
account. Only storage and reconciliation helpers override that identity with
the trusted root account.
Failed setup and verification diagnostics also redact the active opaque
sandbox ID and envd access token before the final 4 KiB tail is selected. A
known value split by the streaming tail boundary has its visible suffix
redacted as well.
Creation and recovery share validation. An invalid one-shot lifetime,
unconfigured logical profile, or unsupported network policy returns a handled
provider-neutral error before any provider request.

## Files, Processes, Terminals, And Screens

Regular-file reads use a descriptor-relative helper with non-following opens
and `fstat` on the opened leaf. Replacement writes stage the bounded payload,
record its caller-computed SHA-256 digest, then traverse the trusted root
through non-following directory descriptors and atomically replace the leaf.
The writer and bounded cleanup helper race for one atomic digest-bearing
commit-or-revoke marker. A revocation winner prevents every later rename; a
commit winner lets cleanup finish only after the temporary bytes match the
requested size and digest. If cleanup instead finds the exact replacement
already visible behind a revocation marker, it resolves the configured
non-root workload account, preserves the opened target's mode, reapplies that
owner, and syncs both the file and containing directory before success. Without
a usable workload identity, that recovery remains unconfirmed. Writer exits
that could follow a commit claim use this same reconciliation path;
definitive pre-commit rejections retain their typed errors. If neither outcome
can be confirmed, `FileWriteUnconfirmed`
requires the caller to keep the sandbox fenced rather than retry. After a
durable replacement, a normally completing writer removes its commit marker.
Upload failures and definitive writer rejections also remove and sync their
marker after cleanup succeeds, because no writer can act afterward. An
uncertain writer leaves its atomic fence available for cleanup reconciliation,
even when cleanup proves that the replacement committed. Cleanup failure never
discards the fence. Both helpers run as root and create the marker below
`/var/lib/sandbox-e2b/write-fences`, whose descriptor-relative parent creation
requires root ownership and denies group or other access. A workload process
therefore cannot remove a revocation and let an older writer commit later. The
root writer creates a root-owned `0700` temporary directory beside the target,
verifies its descriptor, ownership, mode, device, and visible inode, and writes
the prepared payload below that descriptor. This keeps the payload on the
destination filesystem for atomic replacement without exposing its directory
entry to the workload. Both the writer and reconciler rename from a held private
directory descriptor, so moving or replacing the directory's visible name
cannot substitute a different inode. They remove the private directory when
its stable name remains available. The writer then assigns the replacement to
the configured workload account. Its commit marker records the expected
ownership and mode so reconciliation can finish that handoff after an
interrupted commit without exposing the temporary bytes to workload tampering.
Process execution is direct-argv and keeps stdout, stderr, deadlines, and
overflow outcomes separate. Streaming start events and process inventory rows
must decode a nonzero operating-system PID before the adapter exposes them.
Each decoded process-data event must contain
exactly one of PTY, stdout, or stderr; multiple populated channels fail as
malformed instead of silently dropping output. Before acquiring sandbox
access, both direct and stateless commands reject an empty executable and more
than 128 KiB across the executable and arguments. Direct stdout and stderr
limits and the combined stateless output limit are each capped at 64 MiB.
Public Connect waits, combined-output commands, split-stream commands,
regular-file reads, and stateless read-only execution use the same 300-second
ceiling; terminal output long polls use a 30-second ceiling. Caller-controlled
bounds are checked before provider access, and every absolute Tokio deadline
uses checked arithmetic. File and maintenance helpers require a normal process
exit;
a default zero exit code on
a signal event is not success. A one-shot process whose collection times out,
overflows, or fails decoding is killed with a bounded cleanup call once its PID
has been observed; persistent terminal connections are left running
intentionally.

The adapter starts every trusted Python file and terminal helper with isolated
module lookup and without Python site initialization. Sandbox files in the
working directory, `PYTHONPATH`, user-site packages, and startup customization
therefore cannot run before descriptor checks, digest verification, cleanup,
or transcript setup.

Before any read, write, or staged image input acquires sandbox access, the
adapter rejects a root that is not an absolute normalized path and a target
that is not a normalized relative path. Both fields are byte-bounded and
reject empty components, dot components, parent traversal, and NUL bytes.

Terminal recovery lists processes by the configured stable tag and never
starts a replacement when recovery finds no match. Inspection and output reads
bind the stored PID to that exact tag. Input and close requests select the tag
inside the provider operation itself, so a process that reuses the stored PID
cannot receive input or be killed. Restored-terminal cleanup also kills by tag,
then requires its maintenance command to exit normally. Terminal log-directory
creation and restored cleanup open every path component relative to a directory
descriptor with symlink following disabled. An intermediate symlink fails the
operation without creating or deleting content through its target. The
root-authenticated transcript wrapper traverses
`/var/lib/sandbox-e2b/terminals` through non-following directory descriptors,
creates a root-owned regular leaf exclusively, and gives that descriptor only
to the tagged root supervisor. The root recorder writes through a private pipe,
which the supervisor clips to the exact remaining byte count while continuing
to drain overflow. The recorder's child closes all private descriptors,
initializes supplementary groups, and drops its UID and GID to the configured
workload account before starting Bash. Root-authenticated reads first require
the exact terminal-derived log path, then open and read the leaf through one
descriptor-relative helper. Terminal discovery, input, and shutdown use the
same root-authenticated supervisor identity. Durable reads share one absolute
provider deadline and coherent cursor/size reporting. The supervisor waits for
the recorder before exiting, so completed transcripts are fully drained.
Terminal creation and recovery reject a provider log limit above the shared
256 MiB regular-file ceiling before acquiring sandbox access, so a transcript
cannot grow beyond what that reader accepts.
Oversized replacement writes fail before acquiring mutating sandbox access.

The public backend conformance process uses `/bin/sh` with a self-contained
script that emits exact stdout and stderr bytes, so a normal E2B image does not
need a test-only executable. Its lifetime probe creates and destroys a bounded
one-shot sandbox without pausing or resuming it.

Image preparation accepts the caller's durably stored source provider
reference. It stages files, runs setup and ordered verification, scrubs the
runtime, and returns the measured size, but never lists or creates a sandbox or
snapshot and never destroys the source. It validates every staged file's path
and size before connecting to the source, so one invalid later file cannot
leave earlier files written. The caller separately records each create intent,
dispatches it once, and uses the adapter's recover-only methods after dispatch
starts. Empty recovery inventory remains `InProgress` and keeps the source
paused, so an eventual-consistency gap cannot replay build commands, allocate a
second paid sandbox, or create a second image. Multiple candidates likewise
keep the source paused for reconciliation. Image scrub sends TERM to both
configured process names, waits for bounded disappearance, escalates to KILL,
and fails unless both names are gone before size measurement. Both image scrub
and restored-terminal cleanup also stop inherited `sandbox-drive-*` credential
helpers with bounded TERM/KILL polling and fail unless their exit is confirmed.
Cache removal opens every parent and child through non-following directory
descriptors. A setup-created symlink in the home or cache-parent path fails the
scrub, while a symlink at or below a cache leaf is unlinked without deleting
its target.
Setup, verification, scrub, and size measurement run through non-login shells,
so staged files or setup commands cannot install a login profile that skips a
later safety phase or fabricates the measured size. Setup and verification keep
the configured workload identity. The size-measurement command authenticates
as root so `du` can traverse private adapter state such as write fences, and it
propagates filesystem traversal and I/O failures rather than accepting a
partial total.

The terminal transcript descriptor and byte limit are installed by the trusted
root supervisor. Its root recorder receives only a private write pipe, and only
the recorder's child drops to the configured workload account before starting
the intended interactive login shell. This keeps profile output and early exits
inside terminal bookkeeping without giving the shell any way to replace,
truncate, or forge earlier transcript bytes.

Screen ensure and capability discovery invoke the configured helper with
bounded streams. Resize accepts only the exact version, width, and height
object with no extra fields and requires a normal helper exit before reporting
success. Port zero is rejected before connecting to a sandbox. A custom
screen-capable template is built and
published by the separate template release project;
`E2B_SCREEN_TEMPLATE_ID` selects one only for the ignored live smoke test.

## Live Tests

The `live-e2b` feature only compiles credentialed tests. Every live test is also
marked ignored, so `cargo test --workspace --all-features` remains offline.
Running an ignored test requires an explicit API key, may incur provider cost,
and executes cleanup for tracked terminals, sandboxes, and snapshots. Every
sandbox request is retained before dispatch. A shared helper performs bounded,
one-second recover-only polling after an uncertain create and registers a
returned provider handle before any restored-terminal cleanup can fail. Final
cleanup retries every still-pending sandbox request before destroying the
recovered or already tracked resource. The lifecycle test applies the same
ownership to snapshot requests, polls recovery after an in-progress or
delivery-ambiguous response, and retries that recovery during cleanup when no
provider snapshot handle was obtained. A separate ignored probe creates,
recovers, and destroys one one-shot sandbox. Live
ingress probes use the same no-redirect rule as production credentialed
clients, so a redirect cannot forward a private-traffic token to another host.
