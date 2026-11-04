# Sandbox Snapshots And Image Preparation

These requirements apply to saved snapshots and image preparation. They are
part of the [sandbox contract](sandbox-contract.md).

## Snapshot Identity And Sources

Snapshot requests follow the source and correlation validation in the
[creation contract](sandbox-contract.md#creation-and-recovery).

Snapshot creation and recovery remain separate operations. Inventory and
inspection validate usable provider identities. Inspection must return the
exact requested identity. Deletion is idempotent.

Snapshot capture accepts a caller-supplied source provider reference. The
source need not have been created through this adapter. The provider must
permit capture from that source. A provider can allow restoration from a saved
snapshot while refusing capture from a disposable sandbox.

## Durable Image Phases

Image construction is a caller-persisted state machine:

1. Record source-create intent, create the source once, and use only
   source-create recovery after that call starts.
2. Persist the source provider reference, prepare the source once, and
   persist the returned measured size. Preparation never allocates, snapshots,
   or destroys a provider resource.
3. Inventory the source-and-correlation pair, persist the complete snapshot
   request and dispatch intent, and dispatch snapshot creation once.
4. Use only snapshot-create recovery for an uncertain, in-progress, or resumed
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

## Preparation Safety

Every staged file must pass all path and size checks before provider access.
Preparation must not allocate a sandbox, create a snapshot, or destroy a source.

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

Trusted interpreter helpers ignore caller-controlled module lookup and startup
customization. Helper success requires a normal process exit.
[File safety](sandbox-file-safety.md) applies to every staged replacement.
[Transport and diagnostics](sandbox-transport-safety.md) apply to phase errors.
