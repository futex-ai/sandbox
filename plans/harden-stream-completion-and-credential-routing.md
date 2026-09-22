# Harden Stream Completion And Credential Routing

## Summary

Close two safety gaps found by the final review of the standalone sandbox
interface and its E2B adapter. A process result must not be trusted until the
provider's final Connect trailer also succeeds, and credentialed envd and
private-port requests must always use the routing domain selected by validated
adapter configuration.

This is a focused hardening follow-up to the completed extraction plan. It does
not change the provider-neutral API or add a provider.

## Milestone 1: Verify Completion And Pin Credential Routing

At the end of this milestone, a completed process is trusted only after its
final Connect trailer succeeds, and provider credentials can be sent only to
the configured sandbox routing domain.

### Review Items

1. **Severity: medium — consume the final Connect trailer after process end.**
   Process execution sends a normal end event and then a separate final trailer
   that can report a provider error. The combined and split-stream collectors
   currently stop as soon as they see the normal end event, so they do not read
   a later malformed or unsuccessful trailer. Doing nothing can report a
   command or safety helper as successful even though the provider's final
   result says it failed. Option A: store the process result, continue reading,
   and return success only after a valid success trailer; also reject a missing
   trailer. Option B: move final-trailer validation into the HTTP transport so
   collectors never receive an unverified completion. **Recommendation: A**,
   because both collectors already decode these frames and can share the same
   explicit completion rule without redesigning the transport.
2. **Severity: high — pin credentialed envd hosts to adapter configuration.**
   The control API returns a routing domain together with a short-lived access
   token. An injected, faulty, or compromised control transport can currently
   return a different but valid-looking domain, and process or private-port
   code will build a credentialed URL from it. Doing nothing can disclose an
   envd or private-traffic credential to the wrong host. Option A: ignore the
   returned domain and always build process and private-port hosts from the
   adapter's validated configured domain. Option B: require the returned domain
   to match configuration exactly and fail otherwise. **Recommendation: A**,
   because configuration is the trusted routing authority and provider response
   data does not need to choose where credentials are sent.

- [x] Record both review findings with severity, context, impact, options, and
      recommendations in simple language that assumes no prior context.
- [x] Add failing regressions first for a process end followed by an error or
      missing Connect trailer in both collectors, and for mismatched control
      domains in process, read-only, and private-port routing.
- [x] Require combined and split-stream process completion to consume one valid
      success trailer after the process end event.
- [x] Derive every credentialed envd and private-port hostname from the
      adapter's validated configured sandbox domain.
- [x] Update the public contract, adapter documentation, and crate README for
      verified process completion and configuration-pinned credential routing.
- [x] Run focused regressions, formatting, Clippy, the full workspace test
      suite, the file-length lint, smoke coverage, and `cargo xtask check`.
- [x] Audit tracked files for prohibited legacy terms, secrets, artifacts,
      whitespace errors, and unrelated edits.
- [ ] Commit and push the fixes, confirm GitHub CI, then run a clean post-push
      implementation review without changing the worktree.
- [ ] After a clean review, record plan completion and move this plan from
      Active to Completed in `plans/README.md`.
