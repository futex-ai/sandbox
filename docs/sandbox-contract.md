# Sandbox Contract

This document defines the behavior owned by `sandbox-interface`. It describes
the boundary between a trusted consumer and any concrete sandbox provider; it
does not prescribe application persistence or orchestration.

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

`SandboxLifetime` gives both service and backend create requests one of two
policies. `IdleAutoPause` is the default and preserves the existing resumable
interactive lifecycle. `OneShot { max_lifetime }` never pauses, is not
resumable, and remains running until the consumer destroys it or the provider
timeout destroys it. A one-shot duration must be a whole number of seconds in
`1..=3600`, bounded by `SANDBOX_ONE_SHOT_MAX_LIFETIME`; create and recovery must
reject zero, fractional-second, or larger values before provider access.
Consumers should explicitly destroy completed one-shot work instead of waiting
for the timeout.

Adapters preserve the lifetime kind and, for one-shot sandboxes, its maximum
duration in provider metadata used for recovery. Managed inventory returns
`Some(lifetime)` only when that metadata is complete and valid. Missing,
unknown, or malformed lifetime metadata remains `None`, never an inferred
default. Reacquiring access to a running one-shot sandbox must use a
non-mutating read path. Resume, connect, pause, or any other access operation
must not move its destruction deadline beyond the original `max_lifetime`.

## Backend Requirements

Every `SandboxBackend` implementation must support:

- create, recover, inspect, resume, pause, and destroy;
- managed-sandbox listing with optional consumer, lifetime, and correlation IDs;
- snapshot inventory, creation, ambiguous-delivery recovery, inspection, and
  deletion;
- bounded regular-file reads and replacement writes below a trusted root;
- bounded direct-argv process execution with separate stdout and stderr;
- terminal create/recovery, exact identity inspection, bounded transcript
  reads, input, close, and restored-terminal cleanup;
- call-local HTTP port ingress with a redacted optional credential;
- caller-driven platform image phases for source creation, staged files, setup,
  ordered verification, scrub, measured preparation, snapshot dispatch and
  recovery, and explicit source cleanup; and
- idempotent screen-stack ensure plus exact validated viewport resize.

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

Image construction is a caller-persisted state machine:

1. Record source-create intent, call `create_sandbox` once, and use only
   `recover_sandbox_create` after that call starts.
2. Persist the source provider reference, call `prepare_image` once, and
   persist the returned measured size. Preparation never allocates, snapshots,
   or destroys a provider resource.
3. Inventory the source-and-correlation pair, persist the complete snapshot
   request and dispatch intent, and call `create_snapshot` once.
4. Use only `recover_snapshot_create` for an uncertain, in-progress, or resumed
   dispatch. Zero candidates remain in progress; multiple candidates require
   reconciliation. If snapshot capture pauses the source, both outcomes keep it
   paused; only exactly one completed candidate permits reconnecting it. Neither
   outcome may replay preparation or redispatch.
5. Persist the completed snapshot identity and measured size, then destroy the
   source idempotently.

A process failure after intent is recorded but before delivery may require an
operator decision; it is safer to retain the source than guess and duplicate a
paid resource or non-idempotent build side effect. Preparation and recovery
failures never implicitly destroy the caller-owned source.

## Bounds And Failure Safety

Paths must remain under their trusted absolute root and must identify regular
files without following a symlink escape. Replacement writes must bind parent
directories and replace the leaf atomically so concurrent path changes cannot
redirect a write. The caller must calculate the requested payload digest before
upload; both the remote writer and cleanup reconciliation must validate that
identity, not size alone, before replacing the destination. Failed replacement
attempts must make a bounded cleanup attempt for all provider-side staging and
destination-temporary files. An uncertain writer must be fenced by an atomic
revocation or reconciled as an already committed exact replacement. When
cleanup finds the exact replacement already visible, it may report a commit
only after resolving a validated non-root workload identity, preserving the
opened target's mode, applying that owner, syncing the file, and syncing the
destination directory. Without that usable workload identity, the outcome is
unconfirmed. Any writer exit that could follow a commit claim uses the same
reconciliation; definitive pre-commit validation failures preserve their typed
errors. If no outcome can be proven, the backend returns
`FileWriteUnconfirmed`; the caller must not retry on that sandbox until it is
reconciled or destroyed. A normally completed replacement removes its resolved
state marker. Cleanup for an upload failure or definitive writer rejection must
also remove and sync its marker after all cleanup succeeds, because no writer
remains; cleanup failure must leave that marker in place. An uncertain writer
retains an atomic marker as its fence even when reconciliation proves that the
replacement committed.
The writer and reconciler must create that marker as a trusted identity in
storage the workload cannot traverse, unlink, or replace; a marker in a shared
temporary directory is not a valid fence. Using a trusted writer for that
private marker must not leave the replaced workload file owned by the trusted
identity. The verified temporary inode must remain outside workload control
until its atomic rename. A provider may satisfy both that isolation and the
same-filesystem rename requirement by creating a trusted `0700` directory next
to the destination, retaining its verified descriptor, and renaming a private
payload from that descriptor; a workload rename of the directory entry must
not change the source inode. Cleanup must validate the same private directory
before using it. An interrupted commit must retain enough trusted state to
finish the intended ownership and mode handoff during reconciliation.
File transfers are capped at 256 MiB. Every file in a multi-file
image-preparation request must pass its path and size checks before the backend
acquires provider access or writes any earlier file. Provider response and
process output limits are enforced while bytes are consumed. A streaming frame
decoder must validate the bounded frame header before retaining the rest of a
provider chunk, and its partial-frame buffer may retain only the current
allowed frame. A streaming protocol's end marker must also be decoded:
malformed metadata or a reported application error must fail collection rather
than look like an ordinary completion. Once a process end event is observed,
collection must continue until that final marker succeeds; stream exhaustion or
deadline expiry before the marker is a malformed completion. A bounded one-shot
process must be terminated when collection fails after its PID is known. Credentialed HTTP
clients must not follow redirects, and credentials may be attached only after
the exact destination host is validated. Provider-returned routing fields are
not authorities: credentialed process, read-only, and private-port hosts must
come from validated adapter configuration even when a control transport is
injected. A public concrete client must reject
a non-HTTPS or non-root API origin, an empty or padded API key, an invalid
routing domain, and a zero idle timeout before it constructs its credentialed
transport. Empty opaque provider identifiers and
identifiers equal to `.` or `..` must fail before authenticated route
construction. Sandbox and snapshot creation and inventory must also reject
returned identities that their later provider routes cannot use. For E2B, a
sandbox ID must fit the lowercase DNS label used for envd. An accepted mutation
remains delivery-ambiguous, while an invalid inventory row is provider
unavailability. Snapshot inspection must reject a returned provider ID that
differs from the requested ID.
Port zero, empty required
text, oversized values, unknown
profiles, and unsupported network policies fail before provider dispatch. A
direct or stateless process command cannot be empty, and its command and
arguments total at most 128 KiB. Each direct stream or combined stateless
output limit is at most 64 MiB. Every direct, stateless read-only, or
process-transport duration is at most 300 seconds. A terminal output long poll
is at most 30 seconds. A terminal create or recovery request cannot set its
provider transcript limit above the shared 256 MiB regular-file ceiling. These
bounds must be checked before acquiring provider sandbox access, and absolute
deadlines must use checked arithmetic so no caller duration can panic. In
particular, an
oversized replacement write must fail before connecting to or resuming its
sandbox.
Helper processes may report success only after a normal exit; an exit-code
field accompanying signal termination is not a successful completion.
Trusted interpreter helpers must ignore caller-controlled module search paths,
startup customization, and working-directory modules. User files and inherited
language environment settings must not run code before the helper's own logic.
Each provider process start event and process inventory row must contain a
nonzero operating-system PID. Each process-data event must contain exactly one
of PTY, stdout, or stderr output; an event with no channel or multiple channels
is malformed.
Inherited credential or drive helpers must be stopped with bounded escalation,
and maintenance or image preparation fails unless their exit is confirmed.
Provider-owned image verification, scrub, and measurement must not load a
user-controlled login profile before executing; such a profile could otherwise
skip a safety command or forge its result. Image-size traversal and I/O errors
must fail the measurement rather than return a partial total. Measurement may
use a trusted process identity when adapter-private storage is intentionally
inaccessible to the workload; user-authored setup and verification remain on
the configured workload identity. Image cache cleanup must open every parent
without following symlinks and fail if an intermediate component is a symlink;
child symlinks may be unlinked but their targets must never be traversed.
Provider terminal storage creation and restored cleanup must traverse absolute
paths through non-following directory descriptors. An intermediate symlink
must fail closed without creating or removing anything through its target.
File roots and relative paths must pass lexical shape and byte-length checks
before an adapter acquires provider access.

Terminal input and close operations must select the durable terminal identity
atomically in the provider mutation. A separate list-then-mutate check is not a
sufficient identity fence because a numeric process ID can be reused between
the two calls. A successful transport status is not enough to prove terminal
input delivery: the provider's typed acknowledgment must be exactly the
expected empty object. Unknown fields or malformed JSON remain
delivery-ambiguous. The same identity rule
applies when killing terminals inherited by a restored sandbox. Provider-side
transcripts enforce the requested byte count exactly, including limits that
are smaller than or not aligned to 1 KiB, while the accepted limit stays within
the shared 256 MiB reader ceiling. The
provider must create, retain, and read the transcript as a trusted identity in
storage inaccessible to the workload. A tagged trusted supervisor keeps the
storage descriptor, clips bytes from a private recorder pipe, drains overflow,
and waits for recorder exit. The recorder's child closes every private
descriptor, drops to a validated non-root workload identity, and only then
starts the interactive login shell. Reads must derive and verify the same
terminal-owned path before provider access. Process discovery, input, and
shutdown must use the trusted supervisor identity as well. Transcript capture
starts before that shell, so login-profile output and exits remain captured
without allowing the shell to replace, truncate, or forge the stored
transcript.

Before the interactive shell can run or exit, the provider must durably record
the terminal's provider identity, consumer terminal ID, create-operation ID,
and atomic process selector in the same trusted storage class as the
transcript. Create and recovery must securely initialize that storage before
attempting an identity read. The final record name must remain absent while
its private inode is written and synced, then be published atomically without
replacing another record and followed by a directory sync. Recovery validates
that versioned record against the original request. If the process has already
disappeared, recovery and inspection return the recorded provider reference and
`Exited`; they never allocate a
replacement. Unknown record versions, malformed records, identity conflicts,
and duplicate selectors fail closed. A live legacy terminal without a record
remains discoverable by its exact selector, but an exited legacy terminal has
no recoverable provider identity. Input rejects an exited terminal. Explicit
close stays idempotent and removes its identity record; restored-sandbox
cleanup removes all retained terminal identity state.
Inspection and output reads must share the same record-aware identity
resolution. They validate any present record even while its exact process is
live, and use selector-only compatibility only when no record exists. If an
unrelated process reuses an exited terminal's numeric PID, the trusted record
proves the original terminal is `Exited` and its retained transcript remains
readable. A conflicting process using the expected terminal tag still fails
closed.

Sandbox create access is valid only when the provider returns both a nonblank
process credential and a nonblank private-traffic credential. An accepted
create with unusable credentials remains delivery-ambiguous. Ordinary connect
requires the same credentials and maps missing values to retryable provider
unavailability. Read-only access is valid when the provider returns a nonblank
call-local process credential for an already-running sandbox whose automatic
resume is disabled. This read path may omit a private-traffic credential; an
operation that requires that credential must fail instead of mutating a
one-shot sandbox's timeout. A missing or blank process credential is retryable
provider unavailability and must not be sent to the provider process endpoint.

Screen viewport width is `320..=3840`, height is `240..=2160`, and the product
must not exceed 8,294,400 pixels. Resize success requires an exact
version, width, and height acknowledgment with no extra fields, plus a confirmed
normal helper exit. An adapter that cannot confirm resize process termination
returns `ScreenViewportResizeUnconfirmed`; the caller must retain its session
fence and arrange cleanup.

Provider diagnostics returned through handled errors must not contain secret
values or opaque backend handles. Image-command failures must redact every
provider identifier and credential known to the adapter before applying the
final output bound. If streaming capture already omitted earlier bytes, a
leading fragment that can be the suffix of a known sensitive value must also
be redacted. Unknown profile errors do not echo an untrusted profile name.

## Conformance

The public `sandbox_interface::conformance::exercise_backend` harness checks
shared lifecycle, recovery, process, ingress, image, and terminal guarantees.
It includes `exercise_one_shot_lifetime`, which creates, recovers, explicitly
destroys, and idempotently cleans up a bounded one-shot sandbox.
The process probe uses `/bin/sh` plus a self-contained script that emits exact
stdout and stderr bytes; conforming images therefore need a standard shell but
no harness-only executable.
It retains the exact request for every sandbox and snapshot create before
dispatch. After every create result, including synchronous success, it proves
the correlated provider identity through recover-only polling; it never
redispatches creation. A pending recovery waits one second before the next
poll, with no more than 60 waits.

Each returned terminal, snapshot, or sandbox is recorded before later work can
fail. Final cleanup attempts every tracked resource in dependency order and
continues after individual cleanup errors. Requests still awaiting an identity
receive one final recovery attempt so a newly visible resource can be tracked
and removed. If the conformance operation already failed, that primary error
is preserved; cleanup failures are returned only when the operation itself
succeeded. Preparation errors may omit a retained-source diagnostic; when one
is present, the harness requires it to identify the known source. Every
provider adapter should run the harness in addition to its own edge-case and
transport tests.
