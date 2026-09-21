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
sandboxes. Backends preserve that class in creation and inventory. Operations
that are not valid for a class fail with a typed error before dispatch.

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
- platform image realization with staged files, setup, ordered verification,
  scrub, measured snapshot, an explicit source-cleanup reference, and optional
  retained failure runtime; and
- idempotent screen-stack ensure plus exact validated viewport resize.

Creation and snapshot methods separate an initial mutation from recovery. If a
provider cannot prove whether a mutation arrived, it must return an ambiguous
result and use stable correlation data to recover exactly one resource. It must
never silently allocate a duplicate.

Successful image realization and source cleanup are two ordered operations.
The backend first returns the completed image plus its source-sandbox cleanup
reference. The trusted caller durably persists the image identity and measured
size, then idempotently destroys that source. A cleanup failure must never turn
completed realization into an error that replays setup or verification.
Every replay must first look for a completed snapshot using the same source and
correlation identity. If identity remains ambiguous, the backend must retain
the source and return it in `SnapshotReconciliationRequired` when known; it
must not destroy the runtime needed for recovery.

## Bounds And Failure Safety

Paths must remain under their trusted absolute root and must identify regular
files without following a symlink escape. Replacement writes must bind parent
directories and replace the leaf atomically so concurrent path changes cannot
redirect a write. Failed replacement attempts must make a bounded cleanup
attempt for all provider-side staging and destination-temporary files. An
uncertain writer must be fenced by an atomic revocation or reconciled as an
already committed exact replacement. If neither can be proven, the backend
returns `FileWriteUnconfirmed`; the caller must not retry on that sandbox until
it is reconciled or destroyed. File
transfers are capped at 256 MiB. Provider response and process output limits
are enforced while bytes are consumed. A bounded one-shot process must be
terminated when collection fails after its PID is known. Credentialed HTTP
clients must not follow redirects, and credentials may be attached only after
the exact destination host is validated. Port zero, empty required text,
oversized values, unknown profiles, and unsupported network policies fail
before provider dispatch. In particular, an oversized replacement write must
fail before connecting to or resuming its sandbox.
Helper processes may report success only after a normal exit; an exit-code
field accompanying signal termination is not a successful completion.

Terminal input and close operations must select the durable terminal identity
atomically in the provider mutation. A separate list-then-mutate check is not a
sufficient identity fence because a numeric process ID can be reused between
the two calls. The same rule applies when killing terminals inherited by a
restored sandbox. Provider-side transcripts enforce the requested byte count
exactly, including limits that are smaller than or not aligned to 1 KiB.

Screen viewport width is `320..=3840`, height is `240..=2160`, and the product
must not exceed 8,294,400 pixels. Resize success requires an exact
acknowledgment and a confirmed normal helper exit. An adapter that cannot
confirm resize process termination returns `ScreenViewportResizeUnconfirmed`;
the caller must retain its session fence and arrange cleanup.

Provider diagnostics returned through handled errors must not contain secret
values or opaque backend handles. Unknown profile errors do not echo an
untrusted profile name.

## Conformance

The public `sandbox_interface::conformance::exercise_backend` harness checks
shared lifecycle, recovery, process, ingress, image, and terminal guarantees.
Every provider adapter should run it in addition to its own edge-case and
transport tests.
