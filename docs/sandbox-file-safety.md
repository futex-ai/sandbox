# Sandbox File Safety

These requirements apply to the file capability and file staging used by image
preparation. They are part of the [sandbox contract](sandbox-contract.md).

## Regular Files And Replacement

Paths must remain under their trusted absolute root and must identify regular
files without following a symlink escape. Replacement writes must bind parent
directories and replace the leaf atomically so concurrent path changes cannot
redirect a write. The caller must calculate the requested payload digest before
upload; both the remote writer and cleanup reconciliation must validate that
identity, not size alone, before replacing the destination. Failed replacement
attempts must make a bounded cleanup attempt for all provider-side staging and
destination-temporary files. An uncertain writer must be fenced by an atomic
revocation or reconciled as an already committed exact replacement. When
cleanup finds the exact replacement already visible, it may report a commit
only after resolving a validated non-root workload identity, preserving the
opened target's mode, applying that owner, syncing the file, and syncing the
destination directory. Without that usable workload identity, the outcome is
unconfirmed. Any writer exit that could follow a commit claim uses the same
reconciliation; definitive pre-commit validation failures preserve their typed
errors. If no outcome can be proven, the backend returns
`FileWriteUnconfirmed`; the caller must not retry on that sandbox until it is
reconciled or destroyed. A normally completed replacement removes its resolved
state marker. Cleanup for an upload failure or definitive writer rejection must
also remove and sync its marker after all cleanup succeeds, because no writer
remains; cleanup failure must leave that marker in place. An uncertain writer
retains an atomic marker as its fence even when reconciliation proves that the
replacement committed.
The writer and reconciler must create that marker as a trusted identity in
storage the workload cannot traverse, unlink, or replace; a marker in a shared
temporary directory is not a valid fence. Using a trusted writer for that
private marker must not leave the replaced workload file owned by the trusted
identity. The verified temporary inode must remain outside workload control
until its atomic rename. A provider may satisfy both that isolation and the
same-filesystem rename requirement by creating a trusted `0700` directory next
to the destination, retaining its verified descriptor, and renaming a private
payload from that descriptor; a workload rename of the directory entry must
not change the source inode. Cleanup must validate the same private directory
before using it. An interrupted commit must retain enough trusted state to
finish the intended ownership and mode handoff during reconciliation.

## Bounds And Validation

File transfers are capped at 256 MiB. Every file in a multi-file image request
must pass its path and size checks before provider access or any earlier write.
An oversized replacement must fail before connecting to or resuming a sandbox.

File roots and relative paths must pass lexical shape and byte-length checks
before an adapter acquires provider access.

Trusted interpreter helpers must ignore caller-controlled module search paths,
startup customization, and working-directory modules. User files and inherited
language settings must not run code before the trusted helper logic.
Helpers report success only after a normal exit. Signal termination with an
exit-code field is not success.

[Transport and diagnostics](sandbox-transport-safety.md) also apply to every
file operation. File-read results remain sensitive data.
