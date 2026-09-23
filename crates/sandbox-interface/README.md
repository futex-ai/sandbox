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
- Define validated per-session Open and deny-by-default egress policies.
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
`SandboxNetworkPolicy::allowlist` accepts at most 64 typed IP, CIDR, or
lowercase DNS destinations, canonicalizes CIDRs, and sorts and deduplicates the
result. IPv4-mapped IPv6 values canonicalize to IPv4 when representable, and
URL-style legacy IP literals cannot masquerade as domains. Domains may use one
leading `*.` label, which matches subdomains at any depth but not the apex, and
are limited to HTTP/80 and TLS/443 hostname matching; other traffic requires an
IP or CIDR rule. A provider-neutral destination kind does not imply that every
adapter can enforce it safely. An adapter rejects the complete policy with
`UnsupportedNetworkPolicy` before provider mutation when the provider cannot
preserve deployment deny rules for one of its destinations. Raw or deserialized
values are revalidated by adapters. Recovery must reject a policy that differs
from the one used to create the correlated sandbox.
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
Trusted direct-process requests may also carry an optional absolute working
directory and an ordered environment map. The working directory is capped at
4,096 bytes and rejects NUL or control characters. Environment names use
`[A-Za-z_][A-Za-z0-9_]*`, values reject NUL, and the map is capped at 256
entries and 64 KiB across name and value bytes. `PATH`, `HOME`, `LD_*`, and
`DYLD_*` remain template-owned. `ProcessRunContextError` preserves the typed
rejection reason without echoing an environment name or value. Process request
`Debug` exposes only typed consumer identifiers, counts, flags, and limits;
command text, paths, and environment contents are omitted even when no
explicit environment entries are supplied.
Stateless read-only execution keeps its explicit working directory and exposes
no environment map.
Streaming process execution accepts the same non-empty argv and split 64 MiB
limits, but its absolute deadline may be up to 3,600 seconds and it requires a
nonzero idle timeout no greater than that deadline. Only new stdout or stderr
output resets idle timing; buffered delivery to a slow consumer does not.
Typed start, stdout, stderr, exit, and one final outcome preserve normal versus
signal exit. `Completed` requires both a provider success trailer and HTTP EOF,
not a successful command exit. A bounded staging queue may instead report
`ConsumerBackpressure` and initiate best-effort termination and cleanup.
Resumable sandboxes must stay available throughout an accepted stream; a
one-shot sandbox cannot have its original maximum lifetime extended.
Multi-file image preparation validates every file path and size bound before
provider access. Image measurement must fail rather than persist a partial
total. `ImageCommandFailure` contains only exit status, retained output byte
count, and capture truncation; the former output snippet and normalization API
are removed. Process and terminal output `Debug` also omits captured contents,
while raw stdout/stderr, PTY output, and transcript data serialization remain
unmasked. Provider references hide their contents in `Debug`, including nested
retained-sandbox errors, while explicit access and serialization preserve the
reference. Provider adapters must enforce transport frame bounds before retaining provider chunks and must decode
the exact typed empty mutation acknowledgment before reporting delivery
success. After a process end is observed, an adapter must also validate the
provider stream's final status instead of accepting an absent or unsuccessful
completion marker.

The public `conformance` module's `exercise_backend` helper exercises creation,
recovery, image preparation, split-stream execution, private ingress, and
terminal identity. It also creates, recovers, and explicitly destroys a bounded
one-shot sandbox. Its process probe invokes `/bin/sh` with a self-contained
`pwd` and environment script, a selected working directory, one environment
entry, and an independent stderr token, so a normal backend image needs no
test-only executable.
The streaming probe uses a separate self-contained `/bin/sh` command and
checks ordered split output and the final outcome.
The separate `exercise_network_allowlist` capability probe is for adapters
that support domain destinations. It requires `/bin/sh` and `curl`, disables
curl startup configuration before any other option, allows one exact domain,
and verifies that another domain cannot return an application response.
Every sandbox create and both snapshot probes retain the exact request and use
bounded recover-only polling, including after an immediate create result.
Recovery waits one second after each pending result, for at most 60 waits, so
asynchronous providers receive a real completion window without a burst of
polling. Every returned resource is tracked before later work begins. Cleanup
attempts every tracked terminal, snapshot, and sandbox even when an earlier
cleanup action fails, while preserving the original operation error when one
already exists.
Retained-source diagnostics are optional, but are checked against the known
source when present. Every adapter should run the main harness alongside
provider-specific transport and failure tests; domain-capable adapters should
also run the network capability probe.

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
state, close retains the identity for final output reads, and unknown record
versions fail closed. Restored cleanup removes retained identity state. Storage
initialization precedes identity lookup, and the final record name becomes
visible only after its contents are synced and atomically published without
replacement. Inspection and output use the same record-aware resolution and
validate any present record even while the process is live.
Selector-only fallback applies only to a record-free live legacy terminal,
keeping an exited terminal's transcript readable after unrelated PID reuse
without accepting a conflicting terminal tag. Only confirmed identity-file
absence permits that fallback. Bounded output reads carry an earlier absolute
completion deadline so provider-side identity helpers reserve both termination
and return time before the outer deadline.

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
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use sandbox_interface::{
    EgressDestination, SandboxId, SandboxLifetime, SandboxNetworkPolicy, ScreenViewportSize,
};

let sandbox_id = SandboxId::new();
let viewport = ScreenViewportSize::new(1280, 720)?;
let network = SandboxNetworkPolicy::allowlist(vec![
    EgressDestination::domain("api.example.com")?,
    EgressDestination::Ip(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10))),
])?;
let lifetime = SandboxLifetime::OneShot {
    max_lifetime: Duration::from_secs(900),
};
lifetime.validate()?;

assert!(!sandbox_id.to_string().is_empty());
assert_eq!(viewport.width(), 1280);
assert!(matches!(network, SandboxNetworkPolicy::Allowlist { .. }));
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
- `src/network.rs` — typed, bounded outbound network policies.
- `src/conformance.rs` — reusable backend conformance harness.
- `src/conformance_image.rs` — durable image phase conformance flow.

### Related Docs

- [Sandbox contract](../../docs/sandbox-contract.md)
- [E2B adapter guarantees](../../docs/e2b-adapter.md)
- [Process data and diagnostics](../../docs/process-diagnostics.md)
