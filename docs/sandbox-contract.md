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

## Backend Requirements

Every `SandboxBackend` implementation must support:

- create, recover, inspect, resume, pause, and destroy;
- managed-sandbox listing with optional consumer correlation IDs;
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
   reconciliation. Neither outcome may replay preparation or redispatch.
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
revocation or reconciled as an already committed exact replacement. A commit is
reported only after syncing the destination directory, including when cleanup
finds the exact replacement already visible. Any writer exit that could follow
a commit claim uses the same reconciliation; definitive pre-commit validation
failures preserve their typed errors. If no outcome can be proven, the backend
returns
`FileWriteUnconfirmed`; the caller must not retry on that sandbox until it is
reconciled or destroyed. A normally completed replacement removes its resolved
state marker, while an uncertain writer retains an atomic marker as its fence.
The writer and reconciler must create that marker as a trusted identity in
storage the workload cannot traverse, unlink, or replace; a marker in a shared
temporary directory is not a valid fence. Using a trusted writer for that
private marker must not leave the replaced workload file owned by the trusted
identity. The verified temporary inode must remain outside workload control
until its atomic rename, and an interrupted commit must retain enough trusted
state to finish the intended ownership and mode handoff during reconciliation.
File transfers are capped at 256 MiB. Every file in a multi-file
image-preparation request must pass its path and size checks before the backend
acquires provider access or writes any earlier file. Provider response and
process output limits are enforced while bytes are consumed. A streaming frame
decoder must validate the bounded frame header before retaining the rest of a
provider chunk, and its partial-frame buffer may retain only the current
allowed frame. A bounded one-shot process must be
terminated when collection fails after its PID is known. Credentialed HTTP
clients must not follow redirects, and credentials may be attached only after
the exact destination host is validated. Empty opaque provider identifiers and
identifiers equal to `.` or `..` must fail before authenticated route
construction. Port zero, empty required text, oversized values, unknown
profiles, and unsupported network policies fail before provider dispatch. A
direct process command cannot be empty; its command and arguments total at
most 128 KiB, each requested stream limit is at most 64 MiB, and its deadline
is at most 300 seconds. These bounds must be checked before acquiring provider
sandbox access. In particular, an oversized replacement write must fail before
connecting to or resuming its sandbox.
Helper processes may report success only after a normal exit; an exit-code
field accompanying signal termination is not a successful completion.
Trusted interpreter helpers must ignore caller-controlled module search paths,
startup customization, and working-directory modules. User files and inherited
language environment settings must not run code before the helper's own logic.
Each provider process-data event must contain exactly one of PTY, stdout, or
stderr output; an event with no channel or multiple channels is malformed.
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
input delivery: the provider's typed acknowledgment must decode successfully,
and a malformed response remains delivery-ambiguous. The same identity rule
applies when killing terminals inherited by a restored sandbox. Provider-side
transcripts enforce the requested byte count exactly, including limits that
are smaller than or not aligned to 1 KiB. The
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

Screen viewport width is `320..=3840`, height is `240..=2160`, and the product
must not exceed 8,294,400 pixels. Resize success requires an exact
acknowledgment and a confirmed normal helper exit. An adapter that cannot
confirm resize process termination returns `ScreenViewportResizeUnconfirmed`;
the caller must retain its session fence and arrange cleanup.

Provider diagnostics returned through handled errors must not contain secret
values or opaque backend handles. Image-command failures must redact every
provider identifier and credential known to the adapter before applying the
final output bound. If streaming capture already omitted earlier bytes, a
leading fragment that can be the suffix of a known sensitive value must also
be redacted. Unknown profile errors do not echo an untrusted profile name.

## Conformance

The public `sandbox_interface::conformance::exercise_backend` harness checks
shared lifecycle, recovery, process, ingress, image, and terminal guarantees.
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
