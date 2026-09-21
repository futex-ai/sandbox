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
Direct process requests require a non-empty command, at most 128 KiB across
the command and arguments, at most 64 MiB for each captured stream, and a
deadline no longer than 300 seconds.
Multi-file image preparation validates every file-size bound before provider
access, and handled diagnostics redact known sensitive values even when one is
split by a bounded-output cutoff.

The public `conformance` module exercises creation, recovery, image
preparation, split-stream execution, private ingress, and terminal identity.
Both its ordinary and image snapshot probes recover in-progress and
delivery-ambiguous outcomes with a bounded number of calls, including the
ordinary probe's recover-only check after synchronous creation. Recovery waits
one second after each in-progress result, for at most 60 waits, so asynchronous
providers receive a real completion window without a burst of polling. Every
source sandbox created by the image probes is cleaned after preparation,
result validation, inventory, snapshot, or intentional-failure errors.
Retained-source diagnostics are optional, but are checked against the known
source when present. Adapters should run the harness alongside
provider-specific transport and failure tests.

Image construction uses explicit durable phases. The caller records source
create intent before calling `create_sandbox`, uses only
`recover_sandbox_create` after that first call begins, persists the recovered
source, and calls `prepare_image` once. After persisting its measured size, the
caller records one snapshot request and dispatches it once; every retry uses
`recover_snapshot_create` with that same request. Only after the completed
snapshot and size are durable may the caller destroy the source. This division
keeps eventual-consistency gaps from allocating another source, replaying build
scripts, or dispatching another snapshot.

Replacement-file failures are safe to retry only after the backend confirms
that the remote writer was revoked. `FileWriteUnconfirmed` means the caller
must keep the sandbox fenced and reconcile or destroy it before another write.

## Quick Start

```rust
use sandbox_interface::{SandboxId, ScreenViewportSize};

let sandbox_id = SandboxId::new();
let viewport = ScreenViewportSize::new(1280, 720)?;

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
