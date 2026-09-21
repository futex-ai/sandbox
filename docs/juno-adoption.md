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

Image realization now returns `source_sandbox_cleanup_ref`. Persist the image
provider reference and measured size before passing that cleanup reference to
the backend's idempotent sandbox destroy operation. Do not translate a cleanup
failure back into an image-realization failure, because retrying realization
could rerun user-authored setup commands after the snapshot already exists.
Reuse the exact realization operation, sandbox, snapshot, and correlation IDs
on recovery. The backend now inventories that correlation before staging files
or running commands, so a crash after provider snapshot completion returns the
existing image without replaying the build.

Handle `SnapshotReconciliationRequired { retained_sandbox: Some(...) }` as a
recoverable retained build, not as ordinary failed-build cleanup. Persist the
source provider identity and keep it available for a later retry or operator
reconciliation. Destroying it loses the evidence needed to adopt a delayed
snapshot. Likewise, `FileWriteUnconfirmed` requires the sandbox to stay fenced
until it is reconciled or destroyed; do not immediately retry the write.

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
