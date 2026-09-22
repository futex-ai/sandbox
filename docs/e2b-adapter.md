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
prefix, absolute screen-helper path, exact helper and agent process names
stopped before an image snapshot, and the non-root workload account. Neutral
defaults use the `user` account. Existing deployments should explicitly supply
their current values so recovery and snapshot cleanup find the right resources
and commands use the account present in their templates. Root, unsafe account
names, unsafe prefixes or paths, shell characters, whitespace, and process
names longer than Linux's 15-byte task-name limit are rejected.

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
Control and envd DNS, connection, timeout, request, decode, body, and
response-stream failures map to retryable provider unavailability for safe
reads. This includes a successful HTTP response whose JSON does not match the
safe operation's response type. A failure that can occur after a process start,
terminal input, or upload was delivered remains delivery-ambiguous until its
operation-specific recovery fence resolves the outcome.
Opaque provider IDs equal to `.` or `..` are rejected before route
construction, so URL normalization cannot move an API-key-authenticated call
outside its intended sandbox or snapshot endpoint.

Sandbox creation filters on exact configured metadata, including the stable
runtime-or-browser consumer value. Managed inventory returns a recognized
consumer class and leaves it absent for resources created before that metadata
was added. Snapshot recovery walks bounded cursor pagination and adopts exactly
one new correlated snapshot.
Repeated cursors, excessive pages, identity mismatches, and multiple candidates
fail closed. If an accepted create response cannot be decoded or omits the
identity or credentials needed to identify the created resource, the result
remains delivery-ambiguous and enters recovery.

Envd routing is derived from adapter configuration, not a response-provided
host. The complete HTTPS URL must parse to the exact configured envd hostname
before the access-token header is added. Access tokens and private-traffic
credentials stay inside call-local types and are redacted from debug output.
Process and file requests explicitly authenticate the configured workload
account. Only storage and reconciliation helpers override that identity with
the trusted root account.
Failed setup and verification diagnostics also redact the active opaque
sandbox ID and envd access token before the final 4 KiB tail is selected. A
known value split by the streaming tail boundary has its visible suffix
redacted as well.
Creation and recovery share validation. An unconfigured logical profile or
unsupported network policy returns a handled provider-neutral error before any
provider request.

## Files, Processes, Terminals, And Screens

Regular-file reads use a descriptor-relative helper with non-following opens
and `fstat` on the opened leaf. Replacement writes stage the bounded payload,
record its caller-computed SHA-256 digest, then traverse the trusted root
through non-following directory descriptors and atomically replace the leaf.
The writer and bounded cleanup helper race for one atomic digest-bearing
commit-or-revoke marker. A revocation winner prevents every later rename; a
commit winner lets cleanup finish only after the temporary bytes match the
requested size and digest, or verify an already visible exact replacement,
then sync the containing directory before success. Writer exits that could
follow a commit claim use this same reconciliation path;
definitive pre-commit rejections retain their typed errors. If neither outcome
can be confirmed, `FileWriteUnconfirmed`
requires the caller to keep the sandbox fenced rather than retry. After a
durable replacement, a normally completing writer removes its commit marker;
an uncertain writer leaves its atomic fence available for cleanup
reconciliation. Both helpers run as root and create the marker below
`/var/lib/sandbox-e2b/write-fences`, whose descriptor-relative parent creation
requires root ownership and denies group or other access. A workload process
therefore cannot remove a revocation and let an older writer commit later. The
root writer keeps the verified temporary inode trusted through its atomic
rename, then assigns the replacement to the configured workload account. Its
commit marker records the expected ownership and mode so reconciliation can
finish that handoff after an interrupted commit without exposing the temporary
bytes to workload tampering.
Process execution is direct-argv and keeps stdout, stderr, deadlines, and
overflow outcomes separate. Each decoded process-data event must contain exactly one of
PTY, stdout, or stderr; multiple populated channels fail as malformed instead
of silently dropping output. Before acquiring sandbox access, the adapter
rejects an empty command, more than 128 KiB across the command and arguments,
a stdout or stderr limit above 64 MiB, or a deadline above 300 seconds. File and
maintenance helpers require a normal process exit; a default zero exit code on
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
to the root recorder. The recorder's child closes all private descriptors,
initializes supplementary groups, and drops its UID and GID to the configured
workload account before starting Bash. Root-authenticated reads first require
the exact terminal-derived log path, then open and read the leaf through one
descriptor-relative helper. Recorder discovery, input, and shutdown use the
same root-authenticated process identity. Durable reads share one absolute
provider deadline and coherent cursor/size reporting. Transcript writers use
the request's exact byte limit rather than a rounded filesystem block limit.
Oversized replacement writes fail before acquiring mutating sandbox access.

Image preparation accepts the caller's durably stored source provider
reference. It stages files, runs setup and ordered verification, scrubs the
runtime, and returns the measured size, but never lists or creates a sandbox or
snapshot and never destroys the source. It validates every staged file's path
and size before connecting to the source, so one invalid later file cannot
leave earlier files written. The caller separately records each create intent,
dispatches it once, and uses the adapter's recover-only methods after dispatch
starts. Empty
recovery inventory remains `InProgress`, so an
eventual-consistency gap cannot replay build commands, allocate a second paid
sandbox, or create a second image. Image scrub sends TERM to both configured
process names, waits for bounded disappearance, escalates to KILL, and fails
unless both names are gone before size measurement. Both image scrub and
restored-terminal cleanup also stop inherited `sandbox-drive-*` credential
helpers with bounded TERM/KILL polling and fail unless their exit is confirmed.
Cache removal opens every parent and child through non-following directory
descriptors. A setup-created symlink in the home or cache-parent path fails the
scrub, while a symlink at or below a cache leaf is unlinked without deleting
its target.
Setup, verification, scrub, and size measurement run through non-login shells,
so staged files or setup commands cannot install a login profile that skips a
later safety phase or fabricates the measured size. The measurement propagates
filesystem traversal and I/O failures rather than accepting `du`'s partial
output.

The terminal transcript descriptor and byte limit are installed by the trusted
root recorder. Only its child drops to the configured workload account, closes
the storage descriptor, and starts the one intended interactive login shell.
This keeps profile output and early exits inside terminal bookkeeping without
giving the shell any way to replace, truncate, or forge earlier transcript
bytes.

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
and executes cleanup for tracked terminals, sandboxes, and snapshots. Every
sandbox request is retained before dispatch. A shared helper performs bounded,
one-second recover-only polling after an uncertain create and registers a
returned provider handle before any restored-terminal cleanup can fail. Final
cleanup retries every still-pending sandbox request before destroying the
recovered or already tracked resource. The lifecycle test applies the same
ownership to snapshot requests, polls recovery after an in-progress or
delivery-ambiguous response, and retries that recovery during cleanup when no
provider snapshot handle was obtained. Live
ingress probes use the same no-redirect rule as production credentialed
clients, so a redirect cannot forward a private-traffic token to another host.
