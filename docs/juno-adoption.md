# Juno Adoption Follow-Up

Adopting this workspace is a separate Juno change after this repository's pull
request is reviewed and merged. Replace the two local workspace paths with the
reviewed Git revision:

```toml
[workspace.dependencies]
sandbox-interface = { git = "https://github.com/futex-ai/sandbox.git", rev = "<reviewed-revision>" }
sandbox-e2b = { git = "https://github.com/futex-ai/sandbox.git", rev = "<reviewed-revision>" }
```

Then update Rust imports to `sandbox_interface` and `sandbox_e2b`, remove the
old local crate members, and run Juno's full workspace checks.

Replace the old monolithic backend image call with durable orchestration. Store
a source-create intent before calling `create_sandbox` once. If that call is
interrupted or reports ambiguous delivery, call only `recover_sandbox_create`;
an empty result means wait or escalate, not dispatch another paid sandbox.
Persist the source provider reference before calling `prepare_image` once, and
persist its measured size before moving to snapshot dispatch.

Before `create_snapshot`, persist the exact source-and-correlation inventory,
snapshot request, and dispatch intent. Call create once. Every restart,
ambiguous response, or in-progress result must use
`recover_snapshot_create` with that same request. Zero candidates remain in
progress, while multiple candidates need operator reconciliation; neither case
may call preparation or snapshot create again. Persist the completed image and
size before destroying the source. A crash after intent but before confirmed
delivery must fail closed for operator action rather than guess and duplicate
work.

`FileWriteUnconfirmed` requires the sandbox to stay fenced until it is
reconciled or destroyed; do not immediately retry the write.

Juno must construct `E2bRuntimeConventions` with the metadata prefix, terminal
tag prefix, screen-helper path, and image helper and agent process names used
by its existing resources. Set the process names through
`with_image_process_names`. Copy those values from the existing adapter during
the cutover; do not rely on this library's neutral defaults until old resources
have been migrated or retired. This preserves recovery and cleanup across the
dependency switch without embedding one consumer's conventions in the shared
library.

Active terminals created with a different durable log directory must be
drained or have their log files migrated before the cutover, because the shared
adapter uses its neutral log location. Apply the same migration rule to any
template-owned cleanup artifacts whose neutral names changed during extraction.
