# Process Data And Diagnostics

Process and terminal data can contain credentials, including values that have
been encoded, split, or changed by the executed program. The library does not
attempt to discover or replace secrets in that data.

## Exact Data

Commands, arguments, working directories, environment names and values, and
terminal input reach execution unchanged after validation. Direct-process
stdout and stderr, read-only command output, PTY output, and saved terminal
transcripts remain unmasked within their existing capture limits. Existing
terminal text normalization and cursor semantics are unchanged.

Trusted consumers deliberately reading these fields receive sensitive data.
Terminal views and transcript serialization preserve their content; consumers
must not copy that content into ordinary application logs or error messages.

## Automatic Diagnostics

Direct, streaming, and stateless read-only process requests, image preparation
requests, and captured-output `Debug` implementations emit only selected metadata:
consumer-owned typed identifiers, counts, limits, whether a working directory
was supplied, deadlines, exit status, and overflow or truncation flags. They
omit command and argument text, paths, environment names and values, input,
and captured output. This applies with and without an environment map, and to
both ordinary and alternate pretty `Debug` formatting.
Streaming command diagnostics show argument counts, output limits, and
deadlines rather than command or argument contents. Stream-event diagnostics
show stdout and stderr byte counts instead of bytes; start, exit, and outcome
events keep their ordinary field diagnostics. Returned event bytes remain exact.

Terminal input, output, and transcript `Debug` follow the same rule. Formatting
a result changes neither its data fields nor its data serialization. Nested
terminal action results inherit the transcript's safe `Debug` behavior.
Provider references hide their contents in `Debug`, including when nested in
retained-sandbox errors; explicit provider dispatch and serialization retain
the original reference.

Environment validation reports a typed reason and code-owned limits. It does
not echo a rejected name or value, including protected `LD_*` and `DYLD_*`
names whose suffix may contain caller data. Existing validation rules and
validation-before-provider-access ordering remain unchanged.

Tracing uses static event text and selected metadata. Passing raw data fields
directly to tracing bypasses the diagnostic contract and is forbidden.

## Image Failures

`ImageCommandFailure` contains only `exit_code`, `exited`, `output_bytes`, and
`output_truncated`. `output_bytes` is the number of retained raw capture bytes,
not the total bytes emitted if capture was truncated. The outer handled error
identifies setup or verification and preserves the verification index and
optional retained sandbox. A missing exit code or `exited = false` does not
imply successful completion.

Image setup and verification still drain output using the existing bounded
capture behavior. Captured text is never copied into handled errors, `Debug`,
or serialized failure metadata. The former `output` field and
`from_captured_output` normalization/redaction API are removed. There is no
secret matching, ANSI stripping, or replacement chain in failure mapping.

An image error no longer includes a command-output snippet. Retained sandboxes
can be inspected through existing trusted operations. Exact failed-build
output is not recoverable from this error; a future detailed-output facility
must expose it separately as sensitive data with explicit consumer access.

## Verification

Tests assert complete diagnostic field sets, typed validation without echoed
input, and nested-error formatting. Cases include overlapping values, terminal
formatting, encoded credentials, partial values, invalid UTF-8, and truncated
capture. Transport and adapter tests separately assert that process output and
terminal transcript bytes remain exact. Transcript serialization must retain
the original text even though `Debug` omits it.
