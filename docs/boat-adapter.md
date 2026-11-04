# Boat Adapter Contract

The Boat adapter implements lifecycle, saved snapshots, processes, files, and
account capacity. It does not require a local Boat CLI. It follows the
[shared sandbox contract](sandbox-contract.md) for each supported capability.

The adapter exposes lifecycle, saved snapshots, processes, files, and account
capacity through independent capability objects.
See the [crate README](../crates/sandbox-boat/README.md) for construction and
the capabilities available in the current implementation.

Resumable lifecycle, image preparation, terminals, port ingress, and screens
are outside this adapter's supported capabilities. Boat's own stop operation
ends processes and retains disk only when snapshots are enabled. It is not
the shared resumable idle-auto-pause lifecycle.
See [Boat concepts](https://docs.boat.dev/concepts.md).

## Authentication And Transport

Boat's API base is `https://boat.dev/api/v1`. Requests use a Bearer API key.
The adapter keeps that key outside the sandbox and redacts it in diagnostics.
It validates the configured origin and provider route segments before adding
credentials. It uses that exact HTTPS origin and the fixed `/api/v1` prefix.
It rejects arbitrary API paths, query parameters, and fragments in configuration.
Credentialed requests never follow redirects. Response bodies, process output,
and operation time are bounded.
See the [API overview](https://docs.boat.dev/api/v1.md).

The configured key needs the actions used by the selected capabilities.
These include account reads, sandbox create/read/delete, and snapshot reads
and writes. Process and file transport also needs its documented actions.
The adapter does not require account administration or access to the
organization-forbidden deletion-operation endpoint.
See [key scopes](https://docs.boat.dev/api-keys/scopes.md).

## Disposable Creation And Recovery

Every adapter create sends one `POST /sandboxes` with these settings:

- `noEnv: true`.
- `snapshots: false`.
- `monitoring: false`.
- The selected provider size and one-shot TTL.
- Stable labels for owner, consumer, operation, and request correlation.
- The saved snapshot reference when the request selects restoration.

The adapter derives `Idempotency-Key` from the durable create operation.
It preserves the same request identity for recovery. It does not perform an
inventory preflight or repeat create after dispatch begins. Recovery reads
provider inventory and adopts exactly one matching resource. Missing inventory
remains in progress. Conflicting or multiple candidates fail closed.
Boat retains create idempotency keys for 24 hours. That limit does not permit
an adapter to replay a create after recovery becomes uncertain.
See [create sandbox](https://docs.boat.dev/api/reference/sandboxes/create-sandbox.md).

Inspection and managed inventory preserve recognized owner, consumer, size,
network, lifetime, and correlation metadata. Missing or malformed optional
metadata stays unknown. Returned IDs must match Boat's documented sandbox ID
format before an authenticated route uses them.

## Lifetimes

The adapter supports bounded one-shot work. It accepts six hours on an eligible
paid account. Boat's maximum TTL is 2,592,000 seconds, or 30 days.
The adapter validates positive whole seconds and this provider maximum before
provider access. It preserves the original requested lifetime in recovery data.

Boat trials permit at most two hours and require auto-stop. A trial refusal
remains typed as `trial_auto_stop_required`. The adapter does not reduce a
six-hour request to two hours. Account capacity remains advisory.
See [auto-stop](https://docs.boat.dev/long-running-tasks/auto-stop.md).

Ordinary process and file access must not resume, extend, or replace the
one-shot sandbox. The adapter must not invoke Boat's integrated agent prompts,
which can extend auto-stop. At expiry, snapshots-disabled sandboxes lose their
machine and disk. Consumers still destroy completed work explicitly.
See [snapshots and monitoring off](https://docs.boat.dev/data-retention/snapshots-and-monitoring-off.md).

## Secrets And Network Boundary

`noEnv: true` excludes account environment variables, secret files, GitHub
credentials, AI logins, and the in-sandbox account API key.
Boat still provides `ASCII_TOKEN`, which can publish ports of that sandbox.
The adapter must not describe a protected sandbox as containing no credentials.
Monitoring off prevents retained command/output history, process inventory,
and resource samples. Live process output remains available.
See [security](https://docs.boat.dev/security.md) and
[snapshots and monitoring off](https://docs.boat.dev/data-retention/snapshots-and-monitoring-off.md).

Boat has no API setting that enforces sandbox egress rules. The adapter can
support `Open`. It must reject `Allowlist` before provider dispatch rather than
claim that workload-controlled firewall rules enforce it. It must also refuse
any configuration whose deployment denies it cannot preserve.
See [Boat network security](https://docs.boat.dev/security.md#network).

Boat's default `user` account has passwordless sudo. It is a trusted controller.
Caller processes use a distinct restricted non-root workload account. The
adapter creates its default restricted account in a normal base image or
validates the configured account. It refuses an incompatible account.
The child loses supplementary groups, trusted descriptors, and inherited
credentials before execution. Linux no-new-privileges prevents later setuid
or file-capability elevation. The workload cannot traverse the controller home
or trusted state. These guarantees apply to restored images as well.
The default controller alone cannot provide the
[shared file-safety boundary](sandbox-file-safety.md).
See [sandbox isolation](https://docs.boat.dev/security.md#isolation-between-sandboxes).

## Account Capacity And Refusals

Capacity reads `GET /limits` for the same configured account or organization
that create will bill. Its `machineTypes` entries contain `type`, `available`,
and `reason`. The adapter filters the published size catalogue through entries
whose `available` value is true. `/limits` does not supply the full resource
and hourly-price catalogue. An unknown size fails explicitly.
See [account limits](https://docs.boat.dev/api/reference/account/get-boat-limits.md).

The published catalogue below was checked on 4 November 2026.
Memory and disk use Boat's published GB units. Hourly rates use USD.

| Size | vCPU | Memory | Disk | USD per hour |
| --- | --- | --- | --- | --- |
| `small` | 2 | 4 GB | 51 GB | 0.018 |
| `default` | 4 | 8 GB | 51 GB | 0.036 |
| `large` | 8 | 16 GB | 111 GB | 0.072 |
| `xlarge` | 16 | 32 GB | 251 GB | 0.200 |

Boat bills running machine time per second. Eligibility can change with the
account, plan, and trial state. The adapter must not invent resource or price
values for a size that this catalogue does not recognize.
See [Boat pricing](https://docs.boat.dev/pricing.md).

Provider error codes determine the handled refusal. HTTP status alone does not
distinguish temporary start, plan-size, lifetime-trial, and billing refusals.

| Provider result | Required shared behavior |
| --- | --- |
| `rate_limited`, `no_ready_machine`, `out_of_capacity` | Temporary start refusal. Preserve `retryAfterSeconds` and `nextStartAt` when supplied. |
| `limit_reached`, `member_limit_reached` | Account or member concurrency refusal. Preserve supplied retry timing without inventing a delay. |
| `machine_class_plan_required`, `trial_machine_class_not_allowed` | Plan-size refusal. |
| `billing_required`, `team_member_cap_reached`, `trial_compute_limit_reached` | Billing or account-allowance refusal with its typed reason. |
| `trial_auto_stop_required` | Typed trial lifetime refusal. |

A refused start does not queue for later execution. A transport failure after
dispatch remains ambiguous and uses recovery. It must not become a temporary
refusal that falsely proves no resource exists.
See [limits and errors](https://docs.boat.dev/limits.md).

## Saved Snapshots

Saved snapshots use `/saved-snapshots`. Automatic sandbox snapshots are a
different provider resource. Create-from-snapshot restores a saved snapshot
into a disposable sandbox with all required create settings.

Snapshots-disabled sandboxes cannot be captured. Every adapter-created sandbox
keeps snapshots disabled, including a sandbox restored from a saved snapshot.
Saved-snapshot capture accepts a separately prepared source with snapshots
enabled in the same Boat account. The caller owns that source and supplies its
provider reference. The adapter does not enable snapshots on a disposable
source or create a hidden capture source.
See [snapshots and monitoring off](https://docs.boat.dev/data-retention/snapshots-and-monitoring-off.md).

Snapshot create dispatches once with a stable correlation name. It must not
replace a saved snapshot from an unrelated operation. Recovery reads the exact
name and verifies its source. `saving` remains in progress. Only `ready`
confirms successful completion. A uniquely identified failed capture returns
its name with `Failed` state so cleanup can delete it. It must not look
successful or cause redispatch.
Deleting the original source does not delete a saved snapshot.
See [save snapshot](https://docs.boat.dev/api/reference/snapshots/save-saved-snapshot.md) and
[list saved snapshots](https://docs.boat.dev/api/reference/snapshots/list-saved-snapshots.md).

## Processes And Files

The adapter preserves the shared direct-argv, execution-context, output-bound,
deadline, idle-timeout, process-identity, completion, and cleanup guarantees.
It must preserve stdout and stderr independently. File transfers preserve
regular-file confinement, digest verification, atomic replacement, trusted
fences, and uncertain-write handling.

Boat's exec API accepts shell strings. Its direct streaming start frame does
not supply an operating-system PID. Detached start supplies both `processId`
and `pid`. Detached attach supports byte offsets and reconnection.
The exec request does not document stdin input. The adapter must not promise
stdin support based on that API.
See [command execution](https://docs.boat.dev/api/reference/agent/execute-sandbox-command.md) and
[detached attach](https://docs.boat.dev/api/reference/agent/attach-command.md).

The implemented transport uses only the configured HTTPS origin. Caller argv,
cwd, and environment travel in a private typed manifest. Trusted launchers do
not interpolate that data into a shell command. Binary output travels in
bounded authenticated-identity base64 frames. Public start events identify the
actual workload child. Boat launcher PID and command identity stay separate.
A release handshake keeps short commands available for their first live attach.
A final acknowledgment keeps the supervisor available until completion can be
validated. Completion requires child exit, trusted success, a normal supervisor
exit, Boat's final exit, and transport EOF. Reconnect retains exact offsets and
partial inner frames. Heartbeats and reconnect do not extend idle time.

Native file calls transfer only protected chunks of at most 4 MiB. They never
read or write a caller's final workload path. The public file bound remains
256 MiB. Trusted reads use one no-follow descriptor traversal and preserve the
full original size. Trusted writes verify the full caller-computed digest
before atomic replacement. Uncertain writes retain their trusted fence and
return `FileWriteUnconfirmed` unless exact commit or revocation is proven.
Resolved transient staging is removed with bounded trusted cleanup.

## Destruction And Conformance

Destruction repeatedly uses `DELETE /sandboxes/{id}` with the exact deletion
confirmation header. It completes only after a confirmed completed response or
terminal 404. Pending deletion remains incomplete. Destruction must not depend
on `/deletion-operations/{id}`. Saved snapshots remain independent resources.
See [sandbox deletion](https://docs.boat.dev/api/reference/sandboxes/permanently-delete-sandbox-data.md).

Offline tests and construction smoke checks make no real Boat or E2B calls.
Boat runs only its supported [capability probes](sandbox-conformance.md).
Snapshot conformance receives a separate capture-enabled source. Fake-server
coverage must also prove rejection of capture from snapshots-disabled sources.
