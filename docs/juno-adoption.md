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
