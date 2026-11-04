# Sandbox Terminals And Screens

These requirements apply to the terminal and screen capabilities. They are part
of the [sandbox contract](sandbox-contract.md).

## Terminal Bounds And Storage

A terminal output long poll is at most 30 seconds. Terminal create and recovery
must not accept a transcript limit above the 256 MiB regular-file ceiling.
Validate these bounds before provider access.

Provider terminal storage creation and restored cleanup must traverse absolute
paths through non-following directory descriptors. An intermediate symlink
must fail closed without creating or removing anything through its target.

## Terminal Identity And Delivery

Terminal input and close operations must select the durable terminal identity
atomically in the provider mutation. A separate list-then-mutate check is not a
sufficient identity fence because a numeric process ID can be reused between
the two calls. A successful transport status is not enough to prove terminal
input delivery: the provider's typed acknowledgment must be exactly the
expected empty object. Unknown fields or malformed JSON remain
delivery-ambiguous. The same identity rule
applies when killing terminals inherited by a restored sandbox. Provider-side
transcripts enforce the requested byte count exactly, including limits that
are smaller than or not aligned to 1 KiB, while the accepted limit stays within
the shared 256 MiB reader ceiling. The
provider must create, retain, and read the transcript as a trusted identity in
storage inaccessible to the workload. A tagged trusted supervisor keeps the
storage descriptor, clips bytes from a private recorder pipe, drains overflow,
and waits for recorder exit. The recorder's child closes every private
descriptor, drops to a validated non-root workload identity, and only then
starts the interactive login shell. Reads must derive and verify the same
terminal-owned path before provider access. Process discovery, input, and
shutdown must use the trusted supervisor identity as well. Transcript capture
starts before that shell, so login-profile output and exits remain captured
without allowing the shell to replace, truncate, or forge the stored
transcript.

Before the interactive shell can run or exit, the provider must durably record
the terminal's provider identity, consumer terminal ID, create-operation ID,
and atomic process selector in the same trusted storage class as the
transcript. Create and recovery must securely initialize that storage before
attempting an identity read. The final record name must remain absent while
its private inode is written and synced, then be published atomically without
replacing another record and followed by a directory sync. Recovery validates
that versioned record against the original request. If the process has already
disappeared, recovery and inspection return the recorded provider reference and
`Exited`; they never allocate a
replacement. Unknown record versions, malformed records, identity conflicts,
and duplicate selectors fail closed. A live legacy terminal without a record
remains discoverable by its exact selector, but an exited legacy terminal has
no recoverable provider identity. Input rejects an exited terminal. Explicit
close stays idempotent and retains its identity record so final transcript
bytes remain readable; restored-sandbox cleanup removes all retained terminal
identity state.
Inspection and output reads must share the same record-aware identity
resolution. They validate any present record even while its exact process is
live, and use selector-only compatibility only when no record exists. If an
unrelated process reuses an exited terminal's numeric PID, the trusted record
proves the original terminal is `Exited` and its retained transcript remains
readable. A conflicting process using the expected terminal tag still fails
closed. Only a typed missing-file result may enable the legacy fallback;
provider-level terminal absence and every other read error must propagate.
During bounded output polling, the identity helper receives an absolute
completion deadline earlier than the outer deadline. The provider transport
derives its execution cutoff by reserving the full termination window, and the
remaining gap lets the completed helper result return to the caller.

## Screens

Screen-stack ensure is idempotent.

Screen viewport width is `320..=3840`, height is `240..=2160`, and the product
must not exceed 8,294,400 pixels. Resize success requires an exact
version, width, and height acknowledgment with no extra fields, plus a confirmed
normal helper exit. An adapter that cannot confirm resize process termination
returns `ScreenViewportResizeUnconfirmed`; the caller must retain its session
fence and arrange cleanup.

[Transport and diagnostics](sandbox-transport-safety.md) also apply to terminal
input, output, identities, and screen operations.
