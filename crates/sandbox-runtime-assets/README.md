# sandbox-runtime-assets

`sandbox-runtime-assets` holds trusted helper source used by the E2B and Boat
adapters. It is an internal workspace crate. Applications depend on the provider
and interface crates instead.

## Responsibilities

- Share the exact atomic regular-file writer and reconciliation source.
- Keep provider transport, credentials, staging, and runtime wiring separate.

## What This Crate Does

The writer replaces a regular file through held directory descriptors. It
verifies the caller's SHA-256 digest before claiming an atomic commit fence.
The reconciler revokes an unclaimed writer or finishes the exact replacement.
Each adapter supplies trusted private state and a non-root workload owner.
These source assets do not perform provider calls by themselves.

## Quick Start

```bash
cargo build -p sandbox-runtime-assets
cargo test -p sandbox-e2b regular_file
cargo test -p sandbox-boat --test trusted_runtime
```

## Development

Keep helper invocation arguments and exit codes stable when extracting assets.
Provider-specific changes belong in the owning adapter. Real filesystem tests
invoke the isolated production helpers under the Boat integration test target.

### Key Code

- `src/lib.rs` exposes the two audited source constants.
- `src/regular_file_write.py` owns atomic replacement and owner handoff.
- `src/regular_file_cleanup.py` owns commit reconciliation and writer revocation.

### Related Docs

- [File safety](../../docs/sandbox-file-safety.md).
- [E2B adapter](../../docs/e2b-adapter.md).
- [Boat adapter](../../docs/boat-adapter.md).
