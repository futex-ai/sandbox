# Juno Adoption Follow-Up

Adopting this workspace is a separate Juno change after this repository's pull
request is reviewed and merged. Replace the two local workspace paths with the
reviewed Git revision:

```toml
[workspace.dependencies]
sandbox-interface = { git = "https://github.com/futex-ai/sandbox.git", rev = "<reviewed-revision>" }
sandbox-e2b = { git = "https://github.com/futex-ai/sandbox.git", rev = "<reviewed-revision>" }
```

Pin Juno's `internal-error` dependency to the Git source and revision in this
workspace's [`Cargo.toml`](../Cargo.toml). The
[`sandbox-interface` README](../crates/sandbox-interface/README.md) gives the
reason.

Then update Rust imports to `sandbox_interface` and `sandbox_e2b`, remove the
old local crate members, and run Juno's full workspace checks.

## Capability And Create Migration

The backend interface now has independent capabilities. Replace dependencies
on a required all-capability backend with `SandboxLifecycle`,
`SandboxResumableLifecycle`, `SandboxSnapshots`, `SandboxImagePreparation`,
`SandboxProcesses`, `SandboxFiles`, `SandboxTerminals`, `SandboxPortIngress`,
`SandboxScreens`, and `SandboxAccountCapacity` as each consumer needs them.
E2B continues to supply all of its existing behavior.
Use `Arc<dyn Capability>` for each injected dependency. Share one
`Arc<E2bSandboxBackend>` at the composition root and coerce its clones into the
required capability objects. Replace `SandboxBackendRegistry` with
`SandboxRegistry`. Resolve `lifecycle` directly and handle `None` from optional
capability methods. Replace `SandboxBackendMock` expectations with the
corresponding capability mock, such as `SandboxLifecycleMock` or
`SandboxProcessesMock`. Remove `DynSandboxBackend`,
`DynSandboxBackendRegistry`, and calls to `E2bSandboxBackend::into_backend`.
Use explicit capability objects in their place.
Run the applicable conformance probes. A provider needs no unsupported
capability to run a supported probe.

Boat adoption in Juno remains separate. This repository's Boat adapter serves
ciproxy's approved scope. This change does not switch Juno's provider or edit
Juno or ciproxy.

E2B and Boat share the internal `sandbox-runtime-assets` package. Cargo resolves
that package from the same reviewed repository revision. Juno needs no direct
dependency or helper invocation change. E2B keeps its existing helper arguments,
defaults, and filesystem behavior.

Every `BackendCreateSandboxRequest` must set its typed `SandboxConsumer` value. Code
that reads `BackendManagedSandbox` must treat a missing consumer as an older
provider resource with unknown class rather than assuming `Runtime`; newly
created resources preserve the value in provider metadata.

Set `size` on every service and backend create request.
Use `None` to keep E2B's existing profile behavior. E2B rejects `Some` with
`Error::UnsupportedSizeSelection` before provider access.
Other providers can accept `SandboxSizeSelection::ProviderSize(String)` or
`SandboxSizeSelection::Resources(SandboxResources)`. Resources contain `vcpus`,
`memory`, and `disk`. Each memory or disk `ResourceAmount` includes an integer
`amount` and explicit `ResourceUnit`.

Use the registry's optional `account_capacity` accessor for
`Arc<dyn SandboxAccountCapacity>`. Its `account_capacity` method returns
`Vec<SandboxSizeOffer>`. Offers include `provider_size`, `resources`, and
`hourly_price`. The price uses `ExactDecimal { coefficient, scale }`, an explicit
currency code, and `SandboxPriceUnit::Hour`. For example, coefficient 18 at
scale 3 means 0.018 currency units per hour. Do not convert prices through
binary floating point. E2B has no account capacity capability.

Account capacity and remaining limits are advisory. The create result decides
whether the provider accepted a start. Match `Error::SandboxStartRefused` and
its `SandboxStartRefusal` value. Handle `Temporary`, `PlanSize`, `Billing`, and
`TrialAutoStopRequired` separately. Temporary refusals preserve a typed reason
and `SandboxStartRetry`, with optional relative `retry_after` and absolute UTC
`next_start_at`. Missing timing stays absent. These refusals prove no start ran
or queued. They require no uncertain-create recovery. Keep recover-only
handling for ambiguous delivery.

Keep the selected lifetime in create intent and recovery requests. Shared
validation no longer imposes a one-hour maximum. E2B still rejects one-shot
durations above 3,600 seconds. Juno must not infer a longer E2B limit from the
shared type. Access must not extend a one-shot sandbox's original deadline.
Remove uses of the shared `SANDBOX_ONE_SHOT_MAX_LIFETIME` constant.
Shared validation now checks positive whole seconds representable as `u32`.
Use `sandbox_e2b::E2B_ONE_SHOT_MAX_LIFETIME` only for E2B-specific policy.

## Durable Image Orchestration

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
may reconnect the paused source, call preparation, or dispatch snapshot create
again. Persist the completed image and size before destroying the source. A
crash after intent but before confirmed delivery must fail closed for operator
action rather than guess and duplicate work.

`FileWriteUnconfirmed` requires the sandbox to stay fenced until it is
reconciled or destroyed; do not immediately retry the write.

## Process Context And Existing Resources

For streaming process calls, Juno's trusted service must call
`StreamProcessRequest::validate_execution_context()` before provider access
and copy `cwd` and `envs` unchanged into `BackendStreamProcessRequest`.
Use `None` and an empty map when no context is selected. The backend repeats
the shared collected-run validation before acquiring provider access. This
repository defines the service trait but contains no service implementation.

Juno must construct `E2bRuntimeConventions` with the metadata prefix, terminal
tag prefix, screen-helper path, and image helper and agent process names used
by its existing resources. Set the process names through
`with_image_process_names`. Set the non-root workload account of the existing
templates through `with_workload_user`. Copy those values from the existing
adapter during the cutover; do not rely on this library's neutral defaults
until old resources have been migrated or retired. This preserves recovery
and cleanup across the dependency switch without embedding one consumer's
conventions in the shared library.

Active terminals created with a different durable log directory must be
drained or have their log files migrated before the cutover, because the shared
adapter uses its neutral log location. Apply the same migration rule to any
template-owned cleanup artifacts whose neutral names changed during extraction.
New terminal create and recovery requests must keep their provider transcript
limit at or below 256 MiB so the shared regular-file reader can always ingest
the completed transcript.
