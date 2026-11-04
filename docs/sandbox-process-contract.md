# Sandbox Process Contract

These requirements apply to the process capability. They are part of the
[sandbox contract](sandbox-contract.md).

## Bounds And Execution Context

Direct, streaming, and stateless process commands must not be empty.
Their executable and arguments total at most 128 KiB.
Each direct stream or combined stateless output limit is at most 64 MiB.
Collected direct, stateless read-only, and non-streaming transport durations
are at most 300 seconds. Incremental execution accepts up to 3,600 seconds.
Its idle timeout must be nonzero and no greater than its deadline.

Resumable sandboxes must stay available through accepted streaming deadlines.
One-shot sandboxes retain their original destruction deadline. Validate these
bounds before provider access. Absolute deadlines use checked arithmetic.

Trusted collected and streaming direct-process callers may select an optional
working directory and environment map. Absent cwd and an empty map preserve
the provider's existing default context. A working directory must be absolute,
at most 4,096 UTF-8 bytes, and contain no NUL or control character. Environment
names must match
`[A-Za-z_][A-Za-z0-9_]*`; values may contain arbitrary UTF-8 except NUL. A map
contains at most 256 entries and at most 64 KiB across the UTF-8 bytes of every
name and value. `PATH`, `HOME`, every `LD_*` name, and every `DYLD_*` name are
template-owned and cannot be overridden. The same interface-owned validation
must run before provider access. Stateless read-only execution retains its
required explicit working directory and does not accept an environment map.
Helper processes may report success only after a normal exit; an exit-code
field accompanying signal termination is not a successful completion.
Trusted interpreter helpers must ignore caller-controlled module search paths,
startup customization, and working-directory modules. User files and inherited
language environment settings must not run code before the helper's own logic.
Each provider process start event and process inventory row must contain a
nonzero operating-system PID. Each process-data event must contain exactly one
of PTY, stdout, or stderr output; an event with no channel or multiple channels
is malformed.

## Streaming Events And Completion

A process stream becomes available after validation and sandbox access succeed.
If connection consumes the budget, it returns one `DeadlineExpired` outcome.
The service request identifies an owned sandbox. Provider references remain
inside the backend boundary.

Requests carry argv, an optional working directory, an environment map,
separate output limits, an absolute deadline, and an idle timeout.
Execution context uses the same validation as collected runs. Validate that
context first, then argv, output limits, deadline, and idle timeout.
All validation must precede provider access.

Events are ordered as `Started { pid }`, zero or more `Stdout(bytes)`
or `Stderr(bytes)`, `Exited { exit_code, exited }`, then exactly one final
`Outcome` followed by EOF. Before start, failure may emit only an outcome.
Signal termination reports `exited: false`; `Completed` requires a provider
success marker and transport EOF, not a successful command exit. Missing or failed
markers, invalid ordering, malformed frames, bytes after the marker, or timer
expiry after `Exited` produces `TransportFailure`. Stdout and stderr overflow
are distinct and emit only the bounded prefix. Silence before `Started` expires
with `IdleTimeout`; EOF without a success marker still produces `TransportFailure`.
The absolute budget begins before sandbox connection. The idle timer starts
before the process transport opens and resets only on fresh stdout or stderr
arrivals, never start, keep-alive, exit, or delayed consumer delivery. Coalesced
HTTP fragments use their receipt time. The provider reader stages at most 32
decoded events separately from the 16-event consumer queue; when staging is
full it reports `ConsumerBackpressure`, so slow consumers cannot postpone timeout or
cleanup. Overflow, backpressure, timeout, and transport failure trigger a
bounded best-effort kill when a PID was decoded and no process end was decoded.
Before staging or yielding, the reader records the first PID and any subsequent
process end from each decoded batch. A failed frame ends that batch's processed
prefix: the failure is staged, but later frames are ignored and cannot
suppress cleanup.
On consumer drop, a decoded but undelivered PID is still eligible for cleanup.
If the start request may have been sent but no PID is known yet, the adapter
keeps the pending open and reader alive for at most three seconds after the
drop, never beyond the absolute deadline, to learn a PID before killing. The
ordinary idle timer does not cut short this bounded cleanup grace. If none
arrives, it releases the provider stream without a kill; a drop before
the open was polled makes no provider call. A decoded process end prevents a
kill even if its event was not delivered. Queued data and any separately
retained final overflow prefix precede the terminal outcome; cleanup does
not wait for consumer capacity.

## Bounded Transport

Provider response and process output limits apply while bytes are consumed.
A frame decoder validates the bounded header before retaining the remainder of
a provider chunk. Partial-frame storage retains only the current allowed frame.

The decoder must validate the provider's terminal status. Malformed metadata or
an application error must fail collection. Bytes after the terminal marker are
malformed. After process exit, collection continues until the validated success
marker and transport EOF. Later bytes, transport failure, or deadline expiry
before EOF are malformed completion, even after a valid marker.
A bounded one-shot process must be terminated when collection fails after its
PID is known.

[Transport and diagnostics](sandbox-transport-safety.md) define sensitive-data
handling and credential rules.
