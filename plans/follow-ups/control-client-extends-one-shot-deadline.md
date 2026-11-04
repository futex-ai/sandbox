# Public control client can extend one-shot deadlines

- **Severity:** High. **Likelihood:** Rare. **Effort:** Medium. **Confidence:** High. **Harder later:** Yes.
- **Kind:** Needs decision.
- **Category:** Product bug.
- **Added:** 2026-11-04, from the branch `conductor/agents-md-sandbox`. It
  holds the open review finding of the deleted plan
  `plans/one-shot-sandbox-lifetime.md`, recorded on 2026-09-22 in commit
  `c1ba8a4`.
- **Location:** `crates/sandbox-e2b/src/control/client.rs:83`,
  `crates/sandbox-e2b/src/control/client.rs:145`,
  `crates/sandbox-e2b/src/control/client.rs:160`,
  `crates/sandbox-e2b/src/control/client.rs:256-261`,
  `crates/sandbox-e2b/src/control/request.rs:45`,
  `crates/sandbox-e2b/src/control/request.rs:49-52`,
  `crates/sandbox-e2b/src/control/types.rs:175-179`.
- **Reason:** The plan that found this problem on 2026-09-22 left the choice
  of the fix to a maintainer, and nobody has chosen yet.

- **Terms:**
  - **One-shot sandbox:** a sandbox created with
    `SandboxLifetime::OneShot { max_lifetime }`, which E2B destroys when that
    maximum lifetime ends.
  - **Lifetime marker:** the metadata entry `<prefix>_lifetime=one_shot` that
    the backend writes at create, such as `sandbox_lifetime=one_shot` with the
    default prefix.
  - **Direct client:** `ReqwestE2bControlApi`, the public E2B control client
    under `crates/sandbox-e2b/src/control/` that a caller can build without the
    backend.
  - **Sandbox detail:** the response of E2B's `GET /sandboxes/{sandboxID}`,
    which the direct client reads before each connect or pause.
  - **`lifecycle.onTimeout`:** the field of sandbox detail that says what E2B
    does when the sandbox times out, `kill` or `pause`.
- **Background:**
  - The contract. `docs/sandbox-contract.md:3-5` says that the doc covers the
    `sandbox-interface` boundary. Lines 22-24 say that a one-shot sandbox
    "never pauses, is not resumable". Lines 34-36 say: "Resume, connect,
    pause, or any other access operation must not move its destruction
    deadline beyond the original `max_lifetime`." Lines 213-214 say:
    "one-shot sandboxes retain their original destruction deadline."
  - The adapter promise. `docs/e2b-adapter.md:99-103` says that the concrete
    control client reads sandbox detail before connect or pause. For a
    running one-shot sandbox, "it never sends connect, resume, pause, or
    timeout updates". `crates/sandbox-e2b/README.md:79-82` and the trait docs
    at `crates/sandbox-e2b/src/control/api.rs:28`, `:35`, and `:42` say the
    same. No doc tells a direct caller to add the marker.
  - Create. `create_sandbox` at `crates/sandbox-e2b/src/control/client.rs:67-112`
    sends a one-shot body at lines 73-95: `autoPause: false`,
    `autoPauseMemory: false`, `autoResume.enabled: false`, and a `timeout`
    equal to `max_lifetime`. It copies the caller's metadata unchanged at
    line 83. It never adds the marker.
  - Later access. `connect_sandbox` at `client.rs:130-133`,
    `connect_sandbox_with_timeout` at lines 135-155, and `pause_sandbox` at
    lines 157-170 read sandbox detail first. Then they call `is_one_shot` at
    lines 256-261. That function checks only the marker under the key of the
    client.
  - Without the marker, connect sends `POST /sandboxes/{id}/connect` with a
    timeout, at `client.rs:36-58`. Pause sends `POST /sandboxes/{id}/pause`,
    at lines 163-169. `crates/sandbox-e2b/src/backend/mapping.rs:110-112`
    says that this connect "resumes paused sandboxes and extends TTL".
  - The key. `ReqwestE2bControlApi::new` at
    `crates/sandbox-e2b/src/control/request.rs:23-47` is public. It hard-codes
    the key `sandbox_lifetime` at line 45. The only setter,
    `with_lifetime_metadata_key` at lines 49-52, is `pub(crate)`.
  - Only `E2bSandboxBackend::new` calls that setter, at
    `crates/sandbox-e2b/src/backend/configured.rs:48-52`. It passes the key
    of the configured metadata prefix. The backend writes the marker under
    the same prefix at `crates/sandbox-e2b/src/backend/sandbox_metadata.rs:34-37`.
    So the path through `E2bSandboxBackend::new` is safe.
  - Public path one, the direct client.
    `crates/sandbox-e2b/src/lib.rs:17-21` exports `ReqwestE2bControlApi`,
    `E2bControlApi`, `ControlCreateSandbox`, and `SandboxMetadata`.
    `ControlCreateSandbox` has public fields at
    `crates/sandbox-e2b/src/control/types.rs:13-31`. A caller can create a
    one-shot sandbox with empty metadata. A later connect then resumes it or
    extends its timeout. A later pause pauses it.
  - Public path two, the backend with an injected client and a custom
    prefix. `E2bSandboxBackend::with_transports` at `configured.rs:66-76` is
    public. So are `E2bAdapterConfig::with_runtime_conventions` at
    `crates/sandbox-e2b/src/config.rs:105` and `E2bRuntimeConventions::new`
    at `crates/sandbox-e2b/src/runtime_conventions.rs:29`. The default
    prefix is `sandbox`, at `runtime_conventions.rs:7`.
  - In path two, a composition root injects a client from
    `ReqwestE2bControlApi::new` and configures a prefix such as `juno`. The
    backend then writes `juno_lifetime=one_shot`, but the client checks
    `sandbox_lifetime`.
  - The backend has no one-shot guard of its own. Its `resume` and `pause`
    call the client directly, at
    `crates/sandbox-e2b/src/backend/sandboxes.rs:151-178`. Process and
    file-write operations connect through `mapping.rs:68-94`. In path two,
    each of these operations can extend the deadline.
  - `docs/juno-adoption.md:52-54` says that Juno, the planned consumer, must
    use the metadata prefix "used by its existing resources".
  - The reverse case. A direct caller can put the marker on an
    idle-auto-pause create. Pause then returns `Unavailable`, at
    `client.rs:160-162`. Connect uses the read path and never extends the
    timeout, so E2B pauses the sandbox when its idle timeout ends. After that,
    connect refuses the paused sandbox, at `client.rs:234-236`. The caller
    cannot resume it.
  - E2B's API. The public OpenAPI file `spec/openapi.yml` of `e2b-dev/E2B` at
    commit `451a696c62d9a2fa764d523f2b1c1c7eb10e9708`, dated 2026-10-26,
    defines the schema `SandboxLifecycle`. Its description is "Sandbox
    lifecycle policy returned by sandbox info".
  - `SandboxLifecycle` has two required fields: `autoResume`, a boolean, and
    `onTimeout`, an enum of `kill` or `pause`. The schema `SandboxDetail` has
    an optional `lifecycle` field of that schema. In that file, see lines
    791-809 and 983-1043.
  - The same file says that connect resumes a paused sandbox, and that "TTL
    is only extended". The file is at
    <https://github.com/e2b-dev/E2B/blob/451a696c62d9a2fa764d523f2b1c1c7eb10e9708/spec/openapi.yml>.
  - The repository decodes only `autoResume`. `SandboxLifecycleBody` at
    `types.rs:175-179` has no `onTimeout` field. `read_access` at
    `client.rs:237-239` returns `Unavailable` when the detail has no
    `lifecycle`.
  - Tests. No test covers the direct client from create to connect or pause.
    - `crates/sandbox-e2b/src/control/_tests_/sandbox_lifetime_tests.rs:71-98`
      creates a one-shot sandbox with empty metadata, at line 78. It checks
      only the lifecycle fields of the body.
    - `crates/sandbox-e2b/src/control/_tests_/read_access_tests.rs:43-68` and
      `:70-89` use a detail that already holds
      `"sandbox_lifetime":"one_shot"`. The second detail has no `lifecycle`.
    - `read_access_tests.rs:91-115` and
      `crates/sandbox-e2b/src/control/_tests_/control_connect_timeout_tests.rs:16-55`
      assert that a sandbox without the marker gets a POST connect. A
      one-shot sandbox without the marker gets the same request.
    - No test covers path two. The live probe at
      `crates/sandbox-e2b/tests/live_e2b.rs:42-52` uses
      `E2bSandboxBackend::new`. It only creates, recovers, and destroys a
      sandbox.
  - History. Commit `dd40f50` of 2026-09-23 added
    `connect_sandbox_with_timeout`, which has the same problem. No branch
    holds a fix.
- **Problem:** The direct client decides that a sandbox is one-shot only from
  a metadata marker that it never writes. Two public paths can create a
  one-shot sandbox without the marker under the key of the client. A later
  connect then extends its deadline, and a later pause pauses it.
- **Impact if no action:** A one-shot sandbox can outlive its maximum
  lifetime, pause, and resume, against the contract lines above. A paid
  sandbox then stays alive after its deadline. Each new caller of the direct
  client, such as the planned Juno adoption, adds code that a later fix must
  check.
- **Root question:** How does every public path of the E2B adapter keep the
  original deadline of a one-shot sandbox, whatever metadata the caller
  supplies?
- **Options:**
  - **A.** Classify each later access from E2B's `lifecycle.onTimeout`.
    Effort: Medium.
    - Decode `onTimeout` in `SandboxLifecycleBody`. Treat `kill` as one-shot
      and `pause` as resumable. Ignore the marker for this choice.
    - Define what happens when the detail has no `lifecycle`. The spec makes
      that field optional, and no requirement covers the case. One choice is
      to fail closed with `Unavailable`, as `read_access` already does.
    - Add regressions from create to connect and from create to pause, for
      both public paths. Add `lifecycle` with `onTimeout` to the test details
      that rely on the marker.
    - Update `docs/e2b-adapter.md:99-103`, `crates/sandbox-e2b/README.md:79-82`,
      and the trait docs at `api.rs:28-43`. `docs/e2b-adapter.md` has 471
      lines. The `documentation` skill from `futex-ai/skills` says: "When you
      touch a doc that passes the target, split it into linked pages in the
      same change." The target is about 250 lines.
    - A also fixes sandboxes that already exist without the marker, and it
      fixes the reverse case.
    - Risk: a sandbox that another tool created with `autoPause: false` also
      counts as one-shot. The client then never extends or pauses it.
  - **B.** The direct client writes and checks the marker itself. Effort:
    Medium.
    - At create, the client writes the marker under its own key from
      `request.lifetime`. It rejects caller metadata whose marker conflicts
      with that lifetime.
    - A public setter or constructor parameter sets the key, so that a
      composition root can match its metadata prefix.
    - B fixes only new creates. Sandboxes that already exist without the
      marker stay exposed.
    - B adds a public API and a new rejection of conflicting metadata.
- **Left out:**
  - Only two approaches are real. The client either learns the lifetime
    from E2B, or it writes and trusts its own marker. Each other approach
    below fails the goal or breaks a requirement that stays.
  - Make the direct client crate-private. It removes a public API that
    `docs/e2b-adapter.md:24-31` documents. A decision of 2026-09-22, in
    commit `dcebe8a`, kept the client public. That decision made the public
    constructor repeat the safety checks instead.
  - Add a one-shot guard to the backend. It fixes only path two, because a
    direct caller does not use the backend.
  - Add a lifetime argument to connect and pause. Safety then still depends
    on a value from the caller. It also changes the public `E2bControlApi`
    trait.
  - Document that a direct caller must add the marker. Path two stays open,
    because no public setter aligns the key of the client with the
    configured prefix.
  - Change nothing. Both paths keep breaking the contract lines above.
- **Wider fix:** A. It removes the dependence on caller metadata for the
  whole class, and it also covers sandboxes without the marker.
- **Previous examples:** Commit `2471b28` of 2026-09-22 made the same public
  client revalidate typed allow destinations before transport, instead of
  trusting the backend. Commit `dcebe8a` of 2026-09-22 made the public
  constructor repeat the configuration checks of the backend.
- **Related:** `plans/follow-ups/docs-miss-decisions-in-force.md`. Both
  change `docs/e2b-adapter.md`, so the first change that lands splits that
  doc. That item also adds the sentence that the control client stays
  public, which supports the first approach under **Left out**.
- **Recommendation:** A, because provider detail stays correct across process
  restarts and custom metadata prefixes, without making safety depend on
  caller-supplied metadata.
- **Action:** ask the user, because the record of 2026-09-22 leaves the fix to
  a maintainer, and option A needs a rule that no requirement covers.

Source record, at the commit before the deletion:
<https://github.com/futex-ai/sandbox/blob/6f4b0e540703ca5657a3bd28dd917a4263cdd0c0/plans/one-shot-sandbox-lifetime.md?plain=1#L80-L100>
