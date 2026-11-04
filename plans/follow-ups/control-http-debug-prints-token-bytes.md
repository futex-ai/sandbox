# Control HTTP types print token bytes in Debug

- **Severity:** High. **Effort:** Small. **Confidence:** Medium. **Harder later:** No.
- **Kind:** Fix after merge.
- **Category:** Security.
- **Added:** 2026-11-04, from the branch `conductor/agents-md-sandbox`. A check
  of the other `Debug` output of the library found it. No plan named it.
- **Location:** `crates/sandbox-e2b/src/control/http.rs:23-36`,
  `docs/e2b-adapter.md:174-176`.
- **Reason:** The branch that found this problem changed only docs, plans,
  and agent rules.

- **Terms:**
  - **Control transport:** the crate-private trait `E2bHttpTransport` at
    `crates/sandbox-e2b/src/control/http.rs:38-42`, whose method `send` takes
    an `HttpRequest` and returns an `HttpResponse`.
  - **Access tokens:** the E2B credentials `envdAccessToken`, for process and
    file calls, and `trafficAccessToken`, for private-port traffic.
  - **Metadata-only `Debug`:** a hand-written `Debug` implementation that
    prints a status, lengths, and flags, but no body bytes.
- **Background:**
  - `HttpRequest` and `HttpResponse` at
    `crates/sandbox-e2b/src/control/http.rs:23-36` are `pub(crate)`. Both
    derive `Debug`.
    - `HttpRequest` has `method`, `path_and_query`, `body: Option<Vec<u8>>`,
      and `ambiguous_on_failure`.
    - `HttpResponse` has `status`, `body: Vec<u8>`, and `next_token`.
  - A derived `Debug` prints a `Vec<u8>` as a list of decimal byte values. A
    reader can decode that list back to the JSON text.
  - `ReqwestE2bHttpTransport::send` at `http.rs:88-163` keeps the body of each
    2xx response, at lines 158-162. For any other status, it returns an empty
    body, at lines 122-128.
  - Three kinds of 2xx body hold access tokens. The create and connect
    responses hold `envdAccessToken` and `trafficAccessToken`.
    `SandboxAccessBody` decodes them at
    `crates/sandbox-e2b/src/control/types.rs:154-161`. The detail response of
    `GET /sandboxes/{sandboxID}` holds `envdAccessToken`. `SandboxDetailBody`
    decodes it at `types.rs:163-173`.
  - List and snapshot responses hold no token. `next_token` is the pagination
    cursor from the `x-next-token` header. It is not a credential.
  - The request side holds no credential. The transport sends the API key in
    the `X-API-KEY` header at `http.rs:101`, not in `HttpRequest`. The request
    bodies at `types.rs:109-152` hold a template ID, non-secret metadata,
    network rules, timeouts, and a snapshot name.
  - The list queries hold the metadata filter, the page size, and the page
    cursor, at `crates/sandbox-e2b/src/control/pagination.rs:30-34`, `:63-67`,
    and `:96-100`. The path of a request can hold the provider sandbox ID, for
    example `/sandboxes/{id}/connect`.
  - The rule. `docs/e2b-adapter.md:174-176` says: "Access tokens and
    private-traffic credentials stay inside call-local types and are redacted
    from debug output." `HttpResponse` falls short of that rule, and the doc is
    right.
  - The decoded types already meet the rule. `SandboxAccessBody` and
    `SandboxDetailBody` derive no `Debug`. `ControlSandboxAccess` at
    `types.rs:47-57`, `ControlSandboxReadAccess` at `types.rs:91-100`, and
    `ProcessConnection` at `crates/sandbox-e2b/src/process/connection.rs:53-63`
    print `[redacted]` for the token and for the sandbox ID.
  - The paths that can format these types today:
    - Production code: none. Both types are `pub(crate)`, so code outside the
      crate cannot reach them. No production code in the crate formats them
      with `{:?}`. Every `tracing` call in the crate uses static text and
      selected fields.
    - Error types: none. The adapter `Error` at
      `crates/sandbox-e2b/src/error.rs` holds data only in
      `Internal(InternalError)`. A failed JSON decode becomes
      `DeliveryAmbiguous` or `Unavailable` and drops the body, at
      `crates/sandbox-e2b/src/control/request.rs:95-99`.
    - Test mocks: unimock 0.6.8 formats the inputs of a mocked call with
      `Debug` only when it reports a mock error, such as a mismatch. For
      `send`, the input is `HttpRequest`, which holds no token. Unimock never
      formats the returned `HttpResponse`.
    - Test assertions:
      `crates/sandbox-e2b/src/control/_tests_/control_http_tests.rs:41-44` and
      `:62-65` print a `Result<HttpResponse, _>` with `{result:?}` when the
      assertion fails. Their local test server sends no body, so no real
      token can appear.
  - So no path prints a real token today. A future `tracing` field, `{:?}`,
    or error type that keeps a response would print one.
- **Problem:** `HttpResponse` derives `Debug`, so `{:?}` prints the raw bytes
  of a create, connect, or detail response, with its access tokens.
  `docs/e2b-adapter.md:174-176` says that these credentials are redacted from
  debug output. `HttpRequest` also prints its raw body and its path, but no
  credential.
- **Impact if no action:** The first diagnostic that formats a control
  response writes live access tokens to the logs. A reader of those logs can
  then run commands in the sandbox or reach its private ports.
- **Root question:** What may the debug output of a control request or
  response show?
- **Options:**
  - **A.** Give both types a metadata-only `Debug`. Effort: Small.
    - Print `HttpResponse { status, body_bytes, has_next_token }` and
      `HttpRequest { method, body_bytes, ambiguous_on_failure }`.
    - Omit `path_and_query`, because the path can hold the provider sandbox
      ID. The other `Debug` output of the crate redacts that ID.
    - Follow `crates/sandbox-e2b/src/process/output_debug.rs:8-53` and its
      tests in `crates/sandbox-e2b/src/process/_tests_/output_debug_tests.rs`.
      Test `{:?}` and `{:#?}` with a body that holds both tokens. Assert that
      neither the token text nor its byte list appears.
    - The two `{result:?}` assertions keep working. No doc changes, because
      `docs/e2b-adapter.md:174-176` already states the rule.
  - **B.** Remove `Debug` from both types. Effort: Small.
    - The assertions at `control_http_tests.rs:41-44` and `:62-65` then need
      new failure messages, because `{result:?}` needs `HttpResponse: Debug`.
    - Unimock then prints `?` for the request in a mismatch message, so a
      failing control test shows less.
  - **C.** Add a crate-private body type whose `Debug` prints only its byte
    length. Use it for the body fields of both types. Effort: Small.
    - Every later type that holds a control body then gets the safe `Debug`.
    - `HttpResponse` can keep its derived `Debug`. `HttpRequest` still needs
      the hand-written `Debug` of A, so that it omits the path.
    - The change stays in `crates/sandbox-e2b/src/control/`. It changes the
      two field types, the transport, `request.rs`, and the body helpers and
      assertions of eight test files.
    - No doc changes.
- **Left out:**
  - Mask the token values inside the body before printing. That is secret
    matching. The diagnostics rules omit content "instead of matching known
    secrets", at `docs/sandbox-contract.md:373-375`. It also misses any
    credential field that E2B adds later.
  - Decode the response inside the transport, so that no raw body is kept. It
    changes `E2bHttpTransport::send` and every mock of it, and C gives the
    same protection with a smaller change.
  - Add a test that formats every crate-private type. It only detects a leak.
    It does not decide what the debug output shows.
  - Change nothing. The rule at `docs/e2b-adapter.md:174-176` stays unmet, and
    the first diagnostic that formats a response leaks live tokens.
- **Wider fix:** C. A later type that holds a control body cannot print its
  bytes.
- **Previous examples:** `ControlSandboxAccess` at
  `crates/sandbox-e2b/src/control/types.rs:47-57`, `ControlSandboxReadAccess`
  at `types.rs:91-100`, and `ProcessConnection` at
  `crates/sandbox-e2b/src/process/connection.rs:53-63` redact the tokens and
  the sandbox ID. The process outputs print only byte counts, at
  `crates/sandbox-e2b/src/process/output_debug.rs:8-53`.
- **Related:**
  - `plans/follow-ups/file-read-results-debug-prints-bytes.md`: the same
    class, raw bytes in a derived `Debug`.
  - `plans/follow-ups/file-read-requests-debug-prints-paths.md`: the same
    class, caller data in a derived `Debug`.
  - Those items change public types and docs. This item changes only
    crate-private types and no doc, so it does not wait for them.
- **Recommendation:** C, because it is the structural fix for the class, it
  is not blocked, and it stays inside the crate-private control module.
- **Action:** fix after merge, apply C, because the branch that found this
  problem changed no code.
