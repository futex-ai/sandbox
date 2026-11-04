# Concurrent first use can break Boat setup

- **Severity:** High. **Likelihood:** Rare. **Effort:** Medium. **Confidence:** High. **Harder later:** No.
- **Kind:** Fix now.
- **Added:** 2026-11-04, from PR #11, review finding 3.
- **Location:** `crates/sandbox-boat/src/trusted_runtime/helpers/bootstrap.py:47-55`, `:78-83`, and `:95-104`.
- **Reason:** The owner prohibited new finding fixes in the current round. This record preserves pending pre-merge work for a separate fix round.

- **Source:** Review.
- **Category:** Product bug.
- **Terms:**
  - **Bootstrap:** The trusted Python helper that prepares an account and private directories before each Boat process or file call.
  - **Workload account:** The restricted Linux account that runs caller code. Bootstrap creates the default `sandbox-workload` account when absent.
  - **UID/GID:** The numeric user and group identities that Linux uses for file ownership and access.
  - **State root:** The root-only `/var/lib/sandbox-boat` directory that holds private state for each call.
  - **flock:** A file lock that the kernel releases when its owning helper exits.
- **Background:** Process and file capabilities permit concurrent calls on one sandbox. Each call runs the bootstrap independently through `crates/sandbox-boat/src/trusted_runtime/attempt.rs`. Runtime calls also accept caller-supplied sandboxes. They do not require an earlier lifecycle call. The documented sequential runner flow writes an input file and then starts a process. Overlapping first calls on a fresh sandbox expose this defect.
- **Problem:** Bootstrap checks whether the workload account exists before it runs `useradd`. Concurrent bootstraps can both see an absent account. One creation can fail because the account now exists. Both creations can also succeed and replace the account UID/GID. The old owner of `/workspace` then differs from the new account, so the owner check rejects later calls.

  The same unlocked initialization also has a staging-directory ownership window. Another bootstrap can observe `.sandbox-boat` after `mkdir` and before `chown`. Its ownership check then rejects the directory. This also affects configured existing accounts. Source inspection identified this window. The review did not reproduce it.
- **Impact if no action:** A valid process or file call can return `BackendUnavailable`. Account replacement can make every later call fail until sandbox destruction. The sandbox continues to bill until destruction or its lifetime ends.
- **Root question:** How do concurrent first calls get one valid workload account and consistent shared bootstrap state?
- **Options:**
  - **A.** Read and validate the account again after a creation conflict. This misses replacement when both `useradd` calls succeed. It can also observe a user before its group exists. Effort: Small.
  - **B.** Serialize shared initialization inside the trusted helper. Create and validate the root-only state root first. Hold an exclusive lock in that root from account lookup through workspace setup. Reuse the trusted `child_lock()` pattern in `crates/sandbox-boat/src/trusted_runtime/helpers/common.py:98-109`. Keep the existing budgets, validation, and cancellation cleanup. Add the exact concurrent-first-use regression with fake account operations or private account files. Effort: Medium.
  - **C.** Serialize each sandbox's bootstrap calls inside one adapter instance with a keyed Rust async lock. Other adapter instances and processes remain unprotected. Queued calls also wait for the full Boat command round trip. Effort: Medium.
- **Left out:**
  - Create-time provisioning alone does not cover caller-supplied sandboxes. Boat's background setup script can still overlap a first call.
  - Requiring a prepared image does not support the stock-image default account creation.
  - Adapter retries cannot repair an existing workspace owner mismatch.
  - Caller serialization leaves the library defect in every caller.
  - Fixed IDs add a deployment value and leave other shared initialization unlocked.
  - Another account tool adds an image dependency and leaves those steps unlocked. B covers these cases with one shared boundary.
  - Change nothing leaves the failure.
- **Wider fix:** B. One locked section prevents races across all shared bootstrap steps and across adapter instances.
- **Previous examples:** None found in history, plans, or earlier reviews. `crates/sandbox-boat/src/trusted_runtime/helpers/common.py:98-109` already uses the trusted file-lock pattern for per-attempt child state.
- **Related:** Preparation failures use the private budget classification in `crates/sandbox-boat/src/trusted_runtime/attempt.rs` and `crates/sandbox-boat/src/process/execution.rs`. Preserve its absolute expiry outcomes. The launch cleanup lease starts after preparation and does not resolve this race.
- **Recommendation:** B. It prevents account replacement and partial shared initialization across all callers of the sandbox.
- **Action:** fix now, apply B. The current round records this finding without fixing it, as the owner instructed. No owner decision is needed.

## Evidence and verification limits

The original controlled probe used the unchanged account-creation block with fake account operations. Three runs each produced these results:

```text
account initialization failed: useradd exit 9
account initialization succeeded
```

The independent check also used real `useradd` with private account files. Simultaneous starts replaced the account in 40 of 40 trials. With start offsets through 30 ms, 36 trials returned exit 9 and four replaced the account. Traces showed UID/GID 999/999 become 998/998. Some traces briefly had a user entry without its group entry.

The real bootstrap ran in a private mount namespace with a private copy of `/etc`. One concurrent bootstrap failed with exit 49 in all 20 trials. Workspace ownership prevented later bootstraps in 18 trials. One diagnostic had account UID 993 and workspace owner 994. It raised `ValueError` at `bootstrap.py:104`.

The helper converts bootstrap failure to exit 49. `Attempt::initialize` converts that exit to `InvalidResponse`. When time remains, preparation returns `BackendUnavailable` through process and file entry points. The affected bootstrap fails before its caller code starts.

The host used shadow-utils 4.9. No Boat image or its account-tool version was inspected. The isolated starts had no network delay. Real network delay narrows the replacement window. Prefix-mode account-file locking differs from the normal glibc lock. The later private-namespace experiment exercised the real bootstrap with normal account files.

One diagnostic script stopped because of its own wait-loop bug. The corrected script completed. All throwaway directories were removed. No host `sandbox-workload` account remained. No real Boat or E2B API call ran.

## Fix boundaries

Keep account, group, sudo, directory-owner, and directory-mode validation. Keep the controller and workload identities separate. Use a trusted root-owned lock that caller code cannot replace. Keep shared initialization under the lock through workspace setup. Release the lock automatically when the helper exits.

Check cancellation while a bootstrap waits for the lock. A cancelled queued helper can otherwise create attempt directories after abandonment. A slow bootstrap already has this timing risk. The lock must preserve the accepted absolute budget and existing cleanup contract.

Do not change real host system accounts in tests. Use controlled account operations or private account files. Test concurrent default-account creation and shared-directory setup through the real helper boundary. Verify one stable account identity and valid workspace ownership. Preserve the existing process and file regressions.
