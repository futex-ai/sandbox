//! Atomic process-selector wire coverage.

use serde_json::json;

use crate::ProcessSelector;

use super::{encode, send_input, signal};

#[test]
fn input_and_signal_encode_pid_or_tag_as_one_atomic_selector() {
    let input = encode(&send_input(
        ProcessSelector::Tag("sandbox-terminal-id".to_owned()),
        b"input".to_vec(),
    ))
    .expect("tag input");
    let signal = encode(&signal(ProcessSelector::Pid(42))).expect("pid signal");

    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&input).expect("input JSON"),
        json!({"process": {"tag": "sandbox-terminal-id"}, "input": {"pty": "aW5wdXQ="}})
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&signal).expect("signal JSON"),
        json!({"process": {"pid": 42}, "signal": "SIGNAL_SIGKILL"})
    );
}
