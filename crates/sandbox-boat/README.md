# sandbox-boat

`sandbox-boat` implements Boat lifecycle, snapshots, processes, files, and capacity
for `sandbox-interface`. Use it in the composition root that selects Boat.
Application services should receive the supported interface trait objects.
`sandbox-e2b` is a separate provider and is not a dependency of this crate.

## Responsibilities

- Create disposable one-shot sandboxes with durable operation identity.
- Recover uncertain starts through reads without another create.
- Inspect, list, and delete managed sandboxes without extending their TTL.
- Capture caller-owned snapshot sources and restore saved snapshots.
- Report account-eligible published sizes and exact USD hourly prices.
- Execute bounded exact argv with separate binary output and incremental events.
- Read confined regular files and replace them through trusted atomic writes.

## What This Crate Does

Each create sends one `POST /sandboxes` with `noEnv: true`, `snapshots: false`,
and `monitoring: false`. It sends the durable operation as `Idempotency-Key`.
The original owner, consumer, profile, requested size, resolved size, deployment,
network, lifetime, source, and correlation values remain in labels.
Recovery verifies one exact match. Missing inventory remains pending.
Conflicting or multiple matches fail closed.

Profiles select a default published Boat size. Explicit sizing can select a
published name or minimum vCPU, memory, and disk amounts. Resource selection
chooses the smallest published size that meets all amounts, using explicit
decimal or binary units. Create performs no inventory or capacity preflight.
Account capacity filters the [published catalogue](../../docs/boat-adapter.md#account-capacity-and-refusals)
through `/limits`. The catalogue was checked on 4 November 2026.
Capacity is advisory. An eligible unknown machine type fails explicitly.
Definitive start refusals preserve typed temporary, plan, billing, and trial
reasons. Supplied retry seconds and UTC times remain separate.

Boat supports `OneShot` with positive whole seconds through 30 days.
Six-hour requests remain six hours. A provider trial refusal stays typed.
This crate supports `Open` egress. It rejects `Allowlist` before provider access.
Configuration rejects deployment deny rules and profiles requiring restricted
public egress because Boat cannot enforce them.

Saved snapshots use their own exact names. Give each capture operation a unique
durable correlation name. Capture requires a caller-owned source with snapshots
enabled. Disposable adapter creates cannot be captured. Existing names are not
overwritten. Recovery checks the source and name. `saving` remains pending.
Only `ready` confirms a successful capture. A known failed capture retains its
name and `Failed` state so cleanup can delete it. The caller retains ownership
of the source.

Sandbox deletion repeats scoped DELETE with `X-Ascii-Confirm-Delete` until a
completed response or terminal 404. It never uses deletion-operation routes.
Saved-snapshot deletion validates its separate deleted acknowledgment.

Transport requires an HTTPS origin or its exact `/api/v1` base. It uses that
origin and prefix for every request. It disables redirects and validates route
identifiers before adding the redacted Bearer key. Control bodies have a 1 MiB
ceiling. Each control request has a 60-second maximum. Deletion has a 60-second
absolute budget. Runtime attach uses its remaining absolute process budget. The optional additional PEM root keeps certificate and host
verification enabled. The same configured organization applies to all requests.

Process execution uses detached commands and live offset-resuming HTTPS attach.
Private typed manifests carry caller argv, cwd, and environment. Only constant
trusted source and generated identifiers appear in provider shell commands.
This avoids shell interpolation and the Linux single-argument limit. Binary
output uses bounded base64 frames with exact stdout and stderr separation.
A public start event carries the actual workload child's PID. It never carries
Boat's outer launcher PID or command ID. Release and final-ack handshakes keep
short commands available with monitoring disabled. Completion validates both
framing layers, normal supervisor exit, and HTTP EOF. Reconnect preserves
partial frames and byte offsets. Only new workload output resets idle time.

The default `user` account remains the trusted controller. Bootstrap protects
its home, creates private staging, and creates a restricted `sandbox-workload`
account when needed. `with_workload_user` selects a compatible existing account.
The workload must have no sudo rules, privileged groups, or trusted storage
access. Its child starts with a fresh environment, closed private descriptors,
cleared groups, and non-root UID/GID. Linux no-new-privileges also blocks setuid
and file-capability elevation. Restored images must preserve this boundary.
The base image needs absolute Python 3, sudo, useradd, and standard Linux PID
and file-descriptor support. Every helper uses isolated Python with site
startup disabled. Incompatible runtime images fail before caller code starts.

Native files only transfer protected chunks of at most 4 MiB. The public
transfer bound remains 256 MiB. Trusted descriptor traversal performs final
reads and replacements. Upload assembly and atomic replacement both verify
the caller's full SHA-256 digest. The shared writer keeps private root-owned
same-filesystem temporary inodes and commit-or-revoke fences. Final files
belong to the non-root workload and preserve an existing file's mode.
An unresolved writer returns `FileWriteUnconfirmed` and retains its fence.
Normal completion and confirmed cleanup remove transient attempt staging.

This crate exposes no terminal, resumable lifecycle, image-preparation,
ingress, or screen implementation.

## Quick Start

```rust,no_run
use std::{collections::BTreeMap, sync::Arc};
use sandbox_boat::{BoatAdapterConfig, BoatProfile, BoatSandboxBackend};
use sandbox_interface::{SandboxAccountCapacity, SandboxFiles, SandboxLifecycle, SandboxProcesses, SandboxSnapshots};

# fn example(key: String) -> Result<(), Box<dyn std::error::Error>> {
let profiles = BTreeMap::from([("general".to_owned(), BoatProfile {
    provider_size: "default".to_owned(),
    allow_public_egress: true,
    denied_destinations: Vec::new(),
})]);
let config = BoatAdapterConfig::new("boat", "https://boat.dev", key, profiles)?
    .with_organization("personal")?;
let backend = Arc::new(BoatSandboxBackend::new(config)?);
let lifecycle: Arc<dyn SandboxLifecycle> = backend.clone();
let snapshots: Arc<dyn SandboxSnapshots> = backend.clone();
let processes: Arc<dyn SandboxProcesses> = backend.clone();
let files: Arc<dyn SandboxFiles> = backend.clone();
let capacity: Arc<dyn SandboxAccountCapacity> = backend;
# Ok(())
# }
```

The sandbox lifetime and process-call budgets are separate. A six-hour sandbox
can host an independent one-job runner. Collected calls remain limited to
300 seconds. Incremental calls remain limited to 3,600 seconds.
After writing the runner input with `write_file`, a short explicit shell argv
can launch it with redirected stdin and output:

```text
/bin/sh -c 'setsid /workspace/runner < /workspace/input > /workspace/stdout 2> /workspace/stderr &'
```

The short shell exits normally. A decoded child end prevents workload-group
termination, so the independent runner continues. The caller reads its real
output files and explicitly destroys the sandbox when the job finishes.
The original sandbox destruction deadline remains unchanged.

Construction makes no provider call. The key must permit the selected account,
sandbox, and saved-snapshot operations. Keep it outside the workload.

## Development

```bash
cargo build -p sandbox-boat
cargo test -p sandbox-boat
cargo test -p sandbox-boat --test trusted_runtime
cargo clippy -p sandbox-boat --all-targets -- -D warnings
cargo xtask smoke-test
```

The API integration tests use production HTTPS clients and a local TLS server.
Trusted helper integrations use disposable local directories, passwordless
sudo, and distinct real controller and workload UIDs. They change no system
account. Missing local isolation support fails the test. Tests need no provider
key and contact no Boat or E2B service.

### Key Code

- `src/backend/configured.rs` composes explicit control and runtime trait objects.
- `src/runtime/configured.rs` implements process and file capability orchestration.
- `src/runtime/client.rs` decodes documented command and private-staging envelopes.
- `src/process/reader.rs` enforces framing, arrival timing, bounded queues, and cleanup.
- `src/trusted_runtime/helpers/` owns isolated bootstrap, supervisor, and file staging.
- `sandbox-runtime-assets` owns the shared atomic writer and reconciler.
- `src/control/api.rs` defines the typed, injectable control boundary.
- `src/transport/http.rs` enforces HTTPS, credentials, and response budgets.
- `src/transport/request.rs` builds only validated provider routes.
- `src/catalogue.rs` owns published resources and exact prices.
- `tests/boat_control/mod.rs` runs HTTP contracts and supported shared probes.

### Related Docs

- [Boat adapter contract](../../docs/boat-adapter.md).
- [Shared sandbox contract](../../docs/sandbox-contract.md).
- [Transport safety](../../docs/sandbox-transport-safety.md).
- [Capability conformance](../../docs/sandbox-conformance.md).
- [Workspace README](../../README.md).
