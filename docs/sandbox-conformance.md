# Sandbox Capability Conformance

Each provider runs the probes for its supported capabilities alongside its
transport and edge-case tests. Unsupported capabilities are not prerequisites.
The probes preserve the requirements of the
[sandbox contract](sandbox-contract.md).

## Probe Boundaries

Lifecycle probes check create, recovery, inspection, managed inventory, and
idempotent destruction. The one-shot probe creates, recovers, explicitly
destroys, and idempotently cleans up bounded one-shot work.

Snapshot probes can receive an explicit caller-owned source reference.
This permits testing a provider whose disposable creates cannot be captured.
The probe must not delete a caller-owned source. It still owns and cleans every
snapshot that it dispatches. A source created by the probe remains tracked for
cleanup under the normal resource-ownership rule.

Snapshot recovery proves identity separately from successful capture.
The probe tracks a uniquely identified failed snapshot before it reports the
failure. Only `Ready` passes the completion check. Cleanup still deletes the
failed resource and retains a caller-owned source.

Image, process, file, ingress, terminal, and screen probes require only their
own capability dependencies. A provider's supported probe keeps every safety
assertion when the required backend interface changes.

## Process And Network Fixtures

The process probe runs one `/bin/sh` command with `/workspace` as its working
directory and one `SANDBOX_PROBE` entry. It checks the exact `pwd` and
environment bytes on stdout plus an independent deterministic token on stderr,
so conforming images need a standard shell but no harness-only executable.
The streaming probe selects cwd `/workspace` and
`SANDBOX_PROBE=stream-environment-map`, then runs `/bin/sh -c` with
`pwd; printf '%s' "$SANDBOX_PROBE"; printf '%s' 'stream-stderr' >&2`.
It requires stdout `/workspace\nstream-environment-map` and stderr
`stream-stderr`, a nonzero PID, output before exit, a completed outcome after
exit, and no event after the outcome. Its environment value differs from the
collected probe's `environment-map`, so reused collected context cannot pass.

The network allowlist probe applies only to providers that support domains.
It creates a sandbox that permits
only `example.com`, uses `/bin/sh -c` and `curl` to require an application
response from that host, and requires a fetch from a different host to exit
unsuccessfully. Both curl commands put `--disable` first so user or system
startup configuration cannot redirect a request or fabricate a policy result.
The probe also proves that recovery rejects a different policy.

## Recovery And Cleanup

Every sandbox and snapshot request is retained before dispatch. After each
accepted or uncertain create, including immediate success, probes prove the correlated identity
through recover-only polling. They never redispatch creation. Pending recovery
waits one second before its next poll, with no more than 60 waits.
Definitive start or unsupported-size refusals stop recovery for that request.
They leave no resource or pending cleanup obligation for the refused start.

Each returned terminal, snapshot, or sandbox is recorded before later work can
fail. Final cleanup attempts every tracked resource in dependency order and
continues after individual cleanup errors. Requests still awaiting an identity
receive one final recovery attempt so a newly visible resource can be tracked
and removed. If the conformance operation already failed, that primary error
is preserved; cleanup failures are returned only when the operation itself
succeeded. Preparation errors may omit a retained-source diagnostic; when one
is present, the harness requires it to identify the known source.

Cleanup ownership does not change after failure. A probe must attempt every
tracked resource even when an earlier cleanup fails.
