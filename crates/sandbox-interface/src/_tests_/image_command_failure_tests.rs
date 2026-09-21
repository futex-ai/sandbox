//! Tests for safe image-command diagnostic normalization.

use crate::{IMAGE_COMMAND_OUTPUT_MAX_BYTES, ImageCommandFailure};

#[test]
fn normalizes_invalid_utf8_controls_and_terminal_sequences() {
    let failure = ImageCommandFailure::from_captured_output(
        b"\x1b[31mbad\x1b[0m\r\nnext\x00\xff",
        None,
        false,
        &[],
    );

    assert_eq!(failure.output.as_deref(), Some("bad\nnext�"));
    assert!(!failure.output_truncated);
}

#[test]
fn redacts_available_sensitive_values_before_return() {
    let failure = ImageCommandFailure::from_captured_output(
        b"token=secret-value",
        Some(1),
        false,
        &["secret-value".to_owned()],
    );

    assert_eq!(failure.output.as_deref(), Some("token=[REDACTED]"));
}

#[test]
fn normalized_output_keeps_a_utf8_bounded_tail() {
    let input = ["prefix", &"🙂".repeat(IMAGE_COMMAND_OUTPUT_MAX_BYTES)].concat();
    let failure = ImageCommandFailure::from_captured_output(input.as_bytes(), Some(2), false, &[]);
    let output = failure.output.expect("bounded output");

    assert!(output.len() <= IMAGE_COMMAND_OUTPUT_MAX_BYTES);
    assert!(output.ends_with('🙂'));
    assert!(failure.output_truncated);
}
