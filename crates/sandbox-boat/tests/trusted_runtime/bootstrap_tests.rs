//! Concurrent first use exercises complete production helpers with private accounts.

use serde::Deserialize;

use super::{account::BOOTSTRAP, fixture::root_python};

const ABANDON: &str = include_str!("../../src/trusted_runtime/helpers/abandon.py");
const FIXTURE: &str = include_str!("bootstrap_fixture.py");
const PROBE: &str = include_str!("bootstrap_probe.py");

#[derive(Debug, Deserialize)]
struct Initialization {
    statuses: Vec<i32>,
    creations: usize,
    account: [u32; 2],
    workspace: [u32; 2],
    stage: [u32; 3],
    attempts: Vec<[u32; 6]>,
}

#[derive(Debug, Deserialize)]
struct Cancellation {
    status: i32,
    abandoned: Vec<i32>,
    remaining: usize,
    followup: i32,
    lock: [u32; 3],
    lock_preserved: bool,
}

fn probe(scenario: &str) -> Vec<u8> {
    let temporary = tempfile::tempdir().unwrap();
    let source = format!("{FIXTURE}\n{PROBE}");
    let output = root_python(&source)
        .args([BOOTSTRAP, ABANDON])
        .arg(temporary.path())
        .arg(scenario)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
fn concurrent_first_bootstraps_keep_one_account_and_workspace_owner() {
    let observed: Initialization = serde_json::from_slice(&probe("account")).unwrap();
    assert_eq!(observed.statuses, [0, 0, 0], "{observed:?}");
    assert_eq!(observed.creations, 1);
    assert_eq!(observed.account, [10001, 10001]);
    assert_eq!(observed.workspace, [10001, 10001]);
    assert_eq!(observed.stage, [10000, 10000, 0o700]);
    assert_eq!(observed.attempts, [[10000, 10000, 0o700, 0, 0, 0o700]; 3]);
}

#[test]
fn concurrent_configured_bootstraps_never_observe_unowned_staging() {
    let observed: Initialization = serde_json::from_slice(&probe("staging")).unwrap();
    assert_eq!(observed.statuses, [0, 0, 0], "{observed:?}");
    assert_eq!(observed.creations, 0);
    assert_eq!(observed.account, [10001, 10001]);
    assert_eq!(observed.workspace, [10001, 10001]);
    assert_eq!(observed.stage, [10000, 10000, 0o700]);
    assert_eq!(observed.attempts, [[10000, 10000, 0o700, 0, 0, 0o700]; 3]);
}

#[test]
fn cancelled_bootstrap_waiting_for_initialization_cannot_recreate_attempt_state() {
    for scenario in ["cancel", "cancel-no-stage"] {
        let observed: Cancellation = serde_json::from_slice(&probe(scenario)).unwrap();
        assert_eq!(observed.status, 49, "{scenario}: {observed:?}");
        assert_eq!(observed.abandoned, [0, 0]);
        assert_eq!(observed.remaining, 0);
        assert_eq!(observed.followup, 0);
        assert_eq!(observed.lock, [0, 0, 0o600]);
        assert!(observed.lock_preserved);
    }
}
