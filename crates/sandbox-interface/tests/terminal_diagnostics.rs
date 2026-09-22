//! Terminal formatting never changes the data returned to terminal consumers.

use sandbox_interface::{
    ActionId, BackendTerminalOutput, TerminalActionResult, TerminalActionState, TerminalId,
    TerminalState, TranscriptWindow,
};

#[test]
fn terminal_output_debug_omits_payload_and_preserves_raw_bytes() {
    let bytes = b"call-local-token\r\n\x1b[31m\xff".to_vec();
    let output = BackendTerminalOutput {
        bytes: bytes.clone(),
        next_offset: bytes.len() as u64,
        total_size: bytes.len() as u64,
        state: TerminalState::Exited,
        exit_code: Some(1),
        overflowed: false,
    };

    for debug in [format!("{output:?}"), format!("{output:#?}")] {
        assert!(!debug.contains("bytes: ["));
        assert!(debug.contains("output_bytes:"));
    }
    assert_eq!(output.bytes, bytes);
}

#[test]
fn nested_transcript_debug_omits_text_but_serialization_preserves_it() {
    let text = "call-local-token\r\n\x1b[31mpassword\x1b[0m";
    let result = TerminalActionResult {
        action_id: ActionId::new(),
        action_status: TerminalActionState::Completed,
        exit_code: Some(1),
        transcript: TranscriptWindow {
            terminal_id: TerminalId::new(),
            terminal_state: TerminalState::Exited,
            offset: 0,
            next_offset: text.len() as u64,
            revision: 1,
            text: text.to_owned(),
            returned_bytes: text.len(),
            truncated: false,
            action_id: None,
            action_status: None,
        },
    };

    for debug in [format!("{result:?}"), format!("{result:#?}")] {
        assert!(!debug.contains("call-local-token"));
        assert!(!debug.contains("password"));
        assert!(!debug.contains("text:"));
    }
    let serialized = serde_json::to_value(&result).expect("serializable terminal result");
    assert_eq!(serialized["transcript"]["text"], text);
    let restored: TerminalActionResult =
        serde_json::from_value(serialized).expect("terminal result");
    assert_eq!(restored, result);
    assert_eq!(result.transcript.text, text);
}
