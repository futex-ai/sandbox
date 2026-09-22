# sandbox-interface

`sandbox-interface` is the provider-neutral contract for sandbox lifecycle,
files, processes, terminals, snapshots, private port ingress, and screen-stack
operations. Application services and provider adapters should share this crate
instead of depending on one another.

## Responsibilities

- Define stable consumer-owned IDs separately from opaque provider references.
- Define handled errors and async traits for sandbox services, backends,
  registries, and read-only access.
- Define bounded data types for file transfer, process output, terminal logs,
  image preparation, ingress credentials, and screen viewport sizes.
- Provide mocks and a public conformance harness that every backend can run.
- Preserve provider-neutral lifecycle, reconciliation, and recovery semantics.

## What This Crate Does

`SandboxBackend` states what a concrete provider must implement.
`SandboxService` defines the higher-level lifecycle boundary used by a trusted
consumer. `SandboxBackendRegistry` selects an implementation by stable backend
ID, while `SandboxReadOnly` exposes a deliberately narrow inspection surface.

Consumer IDs are UUIDv7 values. Provider references remain opaque, so callers
cannot infer E2B or any future provider's identifier format. Bounded request
types and backend adapters reject unsafe paths, oversized commands and files,
invalid ports, and unsupported viewport dimensions before provider dispatch.
Backend sandbox creation also carries the typed `SandboxConsumer` class.
Managed inventory returns `Some(class)` when provider metadata contains a
recognized value and `None` for older or malformed metadata instead of
guessing a class.
After local validation, sandbox creation dispatches its one provider mutation
without a fallible inventory preflight. Once that call starts, only recovery
reads may follow; ambiguous delivery never permits a second create.
Creation also carries `SandboxLifetime`. `IdleAutoPause` is the default for
resumable interactive sessions. `OneShot { max_lifetime }` never pauses or
resumes and must use a whole-second duration from 1 through 3600 seconds; the
consumer destroys it after work and the provider timeout is the fallback.
Backends validate the same bound on create and recovery before provider access.
Managed inventory reports `Some(lifetime)` only for complete valid metadata and
leaves older or malformed resources unknown.
Direct process and stateless read-only execution requests require a non-empty
command, at most 128 KiB across the command and arguments, and a deadline no
longer than 300 seconds. Each direct-process stream and the combined stateless
output are capped at 64 MiB. A terminal output read may wait at most 30
seconds. Terminal create and recovery requests cap the durable transcript at
the same 256 MiB ceiling as readable regular files. Backends reject every
command, output, duration, and transcript bound before acquiring provider
access.
Multi-file image preparation validates every file path and size bound before
provider access. Image measurement must fail rather than persist a partial
total, and handled diagnostics redact known sensitive values even when one is
split by a bounded-output cutoff. Provider adapters must enforce transport
frame bounds before retaining provider chunks and must decode the exact typed
empty mutation acknowledgment before reporting delivery success. After a
process end is observed, an adapter must also validate the provider stream's
final status instead of accepting an absent or unsuccessful completion marker.

The public `conformance` module exercises creation, recovery, image
preparation, split-stream execution, private ingress, and terminal identity.
It also creates, recovers, and explicitly destroys a bounded one-shot sandbox.
Its process probe invokes `/bin/sh` with a self-contained script that emits
exact stdout and stderr bytes, so a normal backend image needs no test-only
executable.
Every sandbox create and both snapshot probes retain the exact request and use
bounded recover-only polling, including after an immediate create result.
Recovery waits one second after each pending result, for at most 60 waits, so
asynchronous providers receive a real completion window without a burst of
polling. Every returned resource is tracked before later work begins. Cleanup
attempts every tracked terminal, snapshot, and sandbox even when an earlier
cleanup action fails, while preserving the original operation error when one
already exists.
Retained-source diagnostics are optional, but are checked against the known
source when present. Adapters should run the harness alongside
provider-specific transport and failure tests.

Trusted image safety phases must run without user-controlled login startup
files. Durable terminal capture must likewise be installed before the one
intended interactive login shell loads a user profile, so a profile cannot
skip bookkeeping or place output outside the bounded transcript. Provider
adapters must keep write-revocation state and terminal transcript storage
outside workload control, and a trusted recorder must not expose its storage
descriptor to the interactive shell. Image cleanup must refuse symlinked
parents instead of traversing them.
The adapter must persist a trusted, versioned terminal identity before the
shell can exit. Recovery and inspection return the same provider reference as
`Exited` when only that identity and transcript remain; input rejects that
state, close removes the identity idempotently, and unknown record versions
fail closed. Storage initialization precedes identity lookup, and the final
record name becomes visible only after its contents are synced and atomically
published without replacement. Inspection and output use the same record-aware
resolution, keeping an exited terminal's transcript readable after unrelated
PID reuse without accepting a conflicting terminal tag.

Image construction uses explicit durable phases. The caller records source
create intent before calling `create_sandbox`, uses only
`recover_sandbox_create` after that first call begins, persists the recovered
source, and calls `prepare_image` once. After persisting its measured size, the
caller records one snapshot request and dispatches it once; every retry uses
`recover_snapshot_create` with that same request. A provider that pauses the
source for capture must keep it paused while recovery finds zero or multiple
candidates and resume it only after exactly one completed candidate is proven.
Only after the completed snapshot and size are durable may the caller destroy
the source. This division keeps eventual-consistency gaps from allocating
another source, replaying build scripts, or dispatching another snapshot.

Replacement-file failures are safe to retry only after the backend confirms
that the remote writer was revoked. `FileWriteUnconfirmed` means the caller
must keep the sandbox fenced and reconcile or destroy it before another write.
The revocation marker itself must be owned by a trusted identity and stored
where the workload cannot unlink or replace it. A reconciler may report an
already-visible exact target as committed only after applying a validated
non-root workload owner, preserving its mode, and syncing both the file and its
directory. Resolved attempts with no possible live writer remove their marker;
uncertain attempts retain it. Provider adapters must also reject process start
or inventory responses whose operating-system PID is zero.

## Quick Start

```rust
use std::time::Duration;

use sandbox_interface::{SandboxId, SandboxLifetime, ScreenViewportSize};

let sandbox_id = SandboxId::new();
let viewport = ScreenViewportSize::new(1280, 720)?;
let lifetime = SandboxLifetime::OneShot {
    max_lifetime: Duration::from_secs(900),
};
lifetime.validate()?;

assert!(!sandbox_id.to_string().is_empty());
assert_eq!(viewport.width(), 1280);
# Ok::<(), sandbox_interface::Error>(())
```

## Development

```bash
cargo test -p sandbox-interface
cargo test -p sandbox-interface --test backend_conformance
cargo test -p sandbox-interface --test read_only_file_contract
cargo clippy -p sandbox-interface --all-targets --all-features -- -D warnings
```

### Key Code

- `src/backend.rs` — provider lifecycle and reconciliation obligations.
- `src/lifetime.rs` — typed lifetime policies and one-shot validation.
- `src/backend_images.rs` — caller-owned image preparation phase types.
- `src/service.rs` — trusted consumer lifecycle boundary.
- `src/backend_files.rs` and `src/process_run.rs` — bounded file/process data.
- `src/read_only.rs` — narrow non-mutating capability.
- `src/screen_stack.rs` — screen capabilities and validated viewport values.
- `src/conformance.rs` — reusable backend conformance harness.
- `src/conformance_image.rs` — durable image phase conformance flow.

### Related Docs

- [Sandbox contract](../../docs/sandbox-contract.md)
- [E2B adapter guarantees](../../docs/e2b-adapter.md)
