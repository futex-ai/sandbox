# Sandbox Transport And Diagnostics

These requirements apply to every supported sandbox capability. They are part
of the [sandbox contract](sandbox-contract.md).

## Credentials And Routes

Credentialed HTTP clients must not follow redirects. Attach credentials only
after validating the exact destination host. Provider-returned routing fields
are not authorities. Credentialed process, read-only, and private-port hosts
come from validated adapter configuration, including with an injected transport.

Public clients reject non-HTTPS origins and empty or padded API keys before
constructing credentialed transport. E2B clients also reject a non-root API
origin. Boat uses the exact configured HTTPS origin and fixed `/api/v1` prefix.
Boat rejects arbitrary API paths, query parameters, and fragments.
Clients with routing-domain or idle-timeout configuration reject an invalid
domain or zero timeout.

Empty provider identifiers and identifiers equal to `.` or `..` fail before
authenticated route construction. Creation and inventory reject returned
identities that their later routes cannot use. E2B sandbox IDs must fit the
lowercase DNS label used for envd. An accepted mutation remains ambiguous.
An invalid inventory row is retryable provider unavailability.
Snapshot inspection rejects a returned ID that differs from the requested ID.

## Separate Sandbox Credentials

These requirements apply when a provider issues separate process and
private-traffic credentials. E2B retains this credential contract:

Sandbox create access is valid only when the provider returns both a nonblank
process credential and a nonblank private-traffic credential. An accepted
create with unusable credentials remains delivery-ambiguous. Ordinary connect
requires the same credentials and maps missing values to retryable provider
unavailability. Read-only access is valid when the provider returns a nonblank
call-local process credential for an already-running sandbox whose automatic
resume is disabled. This read path may omit a private-traffic credential; an
operation that requires that credential must fail instead of mutating a
one-shot sandbox's timeout. A missing or blank process credential is retryable
provider unavailability and must not be sent to the provider process endpoint.

An adapter that authenticates operations through its configured account
credential still keeps that credential outside the sandbox workload.

## Diagnostics

[Process diagnostics](process-diagnostics.md) define the boundary between
sensitive process data and automatic diagnostics. Process, terminal, and
file-write request `Debug`, tracing, and handled image errors expose selected
metadata only. They omit caller-controlled command text, paths, environment
entries, input, and output
contents instead of matching known secrets. Image failures retain exit and
capture facts without output snippets; nested provider references hide their
contents in `Debug`. Environment validation reports typed reasons without
echoing a name or value. Unknown profile errors do not echo an untrusted name.
Raw stdout/stderr, PTY output, and saved terminal transcripts remain unmasked
within their existing bounds. Their explicit data access and transcript
serialization must preserve content, even when it contains a secret.
`WriteFileRequest` formats only its lifecycle operation ID, owner, sandbox ID,
and input byte count. `BackendWriteFileRequest` formats only the input byte
count. Neither includes the root, path, contents, or provider reference;
explicit request fields and written bytes remain exact. File-read results
are outside this file-write diagnostic guarantee.

The exact provider data can remain available through explicit data operations.
Automatic diagnostics must not include credentials, signed URLs, command text,
file contents, or runner input.
