# sandbox-e2b

`sandbox-e2b` is the first concrete implementation of `sandbox-interface`.
Only composition roots should depend on it; application services should keep
using the provider-neutral interface.

## Responsibilities

- Map E2B control-plane lifecycle and snapshot APIs into shared result types.
- Map envd process, PTY, file, terminal-log, and screen operations behind
  injected traits.
- Keep provider IDs, access tokens, traffic credentials, payloads, and errors
  inside the adapter boundary.
- Reconcile ambiguous creation and snapshot delivery without allocating
  duplicate resources.
- Enforce bounded IO, exact provider identity checks, and private port ingress.
- Keep all credentialed provider tests feature-gated and ignored by default.

## What This Crate Does

`E2bSandboxBackend` implements `SandboxBackend`. Production construction builds
an HTTPS control client and envd process transport; `with_transports` accepts
trait-backed test or alternate transports. Construction itself sends no E2B
request.

`E2bAdapterConfig` validates the API origin, API key, idle timeout, logical
profiles, template IDs, and denied IP/CIDR destinations, then keeps those
values externally immutable. Its neutral runtime conventions use the `sandbox`
metadata prefix, `sandbox-terminal-` process-tag prefix,
`/usr/local/bin/sandbox-screen` helper, and neutral image-cleanup process names.
Deployments that must adopt existing resources can supply an
`E2bRuntimeConventions` value; prefixes, the absolute helper path, and exact
cleanup process names are validated before use.

Creates use exact metadata to recover ambiguous delivery. Snapshot recovery
uses bounded, cursor-safe inventory traversal. Terminal identities combine an
E2B PID with the consumer terminal ID; reads verify both values, while input and
close operations use envd's atomic tag selector so PID reuse cannot retarget
them. File reads use one descriptor-relative, non-following helper; writes stage
their payload, bind it to the caller-computed SHA-256 digest, perform one
descriptor-relative atomic replacement below the trusted root, and use an
atomic digest-bearing commit-or-revoke marker to reconcile an uncertain writer
without relying on a timing delay. Both the writer and cleanup path verify the
requested bytes before replacement. An unconfirmed revocation returns a
fencing error instead of pretending the write safely failed.
Oversized writes fail before sandbox connection. Terminal transcript writers
enforce arbitrary byte limits exactly rather than rounding to filesystem
blocks.
One-shot processes are killed when collection times out or fails after
observing their PID. HTTP bodies, process output, and terminal output are
bounded while streaming. Direct process requests are validated before the
adapter acquires sandbox access: commands must be non-empty, combined argv is
capped at 128 KiB, each stream cap is at most 64 MiB, and deadlines cannot
exceed 300 seconds. Failed image-command diagnostics redact the call-local
opaque sandbox ID and envd access token before returning bounded output,
including a sensitive suffix split by the streaming tail boundary. Malformed
process data with zero or multiple output channels is rejected instead of
silently losing bytes.
Credentialed clients, including opt-in live ingress probes, do not follow
redirects, and envd URLs are validated before call-local credentials are
attached. Definitive rejection headers are mapped without waiting for an
unused response body. DNS, connection, timeout, and response-stream failures
remain typed as provider unavailability; failed mutating delivery remains
ambiguous. A successful safe response with malformed JSON is also retryable
provider unavailability, while malformed output after an accepted mutation
keeps the delivery outcome ambiguous.

Image construction is split across the interface's durable phases. E2B
preparation accepts an already persisted source and never creates, snapshots,
or destroys a provider resource. Every staged input size is checked before the
adapter connects or writes the first file. Consumers dispatch source and
snapshot creates once, use only their recovery methods after each dispatch
starts, and persist preparation's measured size before snapshot dispatch.
Empty recovery inventory stays in progress instead of replaying preparation
or allocating another resource. Before measuring a prepared source, configured
image processes must exit after bounded TERM/KILL escalation. Restored-sandbox
cleanup and image preparation apply the same bounded escalation to inherited
drive helpers and fail unless those helpers are confirmed gone. Terminal
log-directory creation and restored cleanup traverse from directory
descriptors with non-following opens; an intermediate symlink makes the
operation fail without touching its target. Image setup, verification, scrub,
and size measurement use non-login shells, so a staged or setup-created profile
cannot skip a later safety phase or forge its result. The terminal transcript
wrapper also starts through a non-login outer shell; only the captured
interactive shell loads the user's login profile.

Screen ensure and resize commands use the configured template helper. Resize
keeps one absolute deadline, reserves cleanup time, and reports an unconfirmed
termination separately so callers do not release a possibly active session.
Compatible custom template publication belongs to the external template
release project, not this crate.

## Quick Start

```rust
use std::collections::HashMap;
use sandbox_e2b::{E2bAdapterConfig, E2bProfile, E2bSandboxBackend};

let config = E2bAdapterConfig::new(
    "e2b",
    "https://api.e2b.app",
    "placeholder-api-key",
    HashMap::from([("general".to_owned(), E2bProfile {
        template: "base".to_owned(),
        allow_public_egress: true,
        denied_destinations: vec!["169.254.169.254/32".to_owned()],
    })]),
    600,
)?;
let _backend = E2bSandboxBackend::new(config)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Development

Credential-free checks do not contact E2B:

```bash
cargo test -p sandbox-e2b
cargo test -p sandbox-e2b --test screen_resize_deadline
cargo clippy -p sandbox-e2b --all-targets --all-features -- -D warnings
```

Live tests are opt-in, ignored, and billable:

```bash
E2B_API_KEY=... cargo test -p sandbox-e2b \
  --features live-e2b --test live_e2b -- --ignored

E2B_API_KEY=... E2B_SCREEN_TEMPLATE_ID=... \
  cargo test -p sandbox-e2b --features live-e2b --test live_e2b \
  live_e2b_private_screen_bridges -- --ignored
```

Every live sandbox create request is stored before dispatch. One shared helper
uses bounded, one-second recover-only polling after an uncertain result and
registers the provider handle before restored-terminal cleanup or other later
work can fail. Final cleanup retries unresolved sandbox recovery before
destroying tracked resources. The lifecycle test applies the same ownership to
snapshot requests and retries recovery when the initial flow did not obtain a
deletable snapshot handle.

### Key Code

- `src/config.rs` — validated profiles and adapter configuration.
- `src/runtime_conventions.rs` — validated deployment and cleanup names.
- `src/backend/configured.rs` — backend construction and trait dispatch.
- `src/backend/image_realization.rs` — one-shot caller-owned image preparation.
- `src/backend/terminal_storage.rs` — non-following terminal path helpers.
- `src/control/` — E2B control API boundary.
- `src/process/` — envd Connect framing and operations.
- `src/backend/sandboxes.rs` — metadata correlation and lifecycle mapping.
- `src/backend/screen_resize.rs` — deadline and termination guarantees.

### Related Docs

- [Sandbox contract](../../docs/sandbox-contract.md)
- [E2B adapter guarantees](../../docs/e2b-adapter.md)
- [E2B snapshot documentation](https://e2b.dev/docs/sandbox/snapshots)
