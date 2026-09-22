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
fn redacts_overlapping_sensitive_values_without_leaving_a_suffix() {
    let failure = ImageCommandFailure::from_captured_output(
        b"token=secret-value",
        Some(1),
        false,
        &["secret".to_owned(), "secret-value".to_owned()],
    );

    assert_eq!(failure.output.as_deref(), Some("token=[REDACTED]"));
}

#[test]
fn redacts_sensitive_values_before_crlf_normalization() {
    let sensitive = "top\r\nsecret".to_owned();
    let failure = ImageCommandFailure::from_captured_output(
        b"token=top\r\nsecret",
        Some(1),
        false,
        &[sensitive],
    );

    assert_eq!(failure.output.as_deref(), Some("token=[REDACTED]"));
}

#[test]
fn redacts_sensitive_values_before_ansi_removal() {
    let sensitive = "\x1b[31msecret\x1b[0m".to_owned();
    let failure = ImageCommandFailure::from_captured_output(
        b"token=\x1b[31msecret\x1b[0m",
        Some(1),
        false,
        &[sensitive],
    );

    assert_eq!(failure.output.as_deref(), Some("token=[REDACTED]"));
}

#[test]
fn redacts_truncated_sensitive_suffix_before_ansi_removal() {
    let sensitive = "prefix\x1b[31msecret\x1b[0m".to_owned();
    let failure = ImageCommandFailure::from_captured_output(
        b"\x1b[31msecret\x1b[0m tail",
        Some(1),
        true,
        &[sensitive],
    );

    assert_eq!(failure.output.as_deref(), Some("[REDACTED] tail"));
    assert!(failure.output_truncated);
}

#[test]
fn redacts_longest_truncated_suffix_before_contained_complete_value() {
    let failure = ImageCommandFailure::from_captured_output(
        b"fix-password123",
        Some(1),
        true,
        &["prefix-password123".to_owned(), "password".to_owned()],
    );

    assert_eq!(failure.output.as_deref(), Some("[REDACTED]"));
    assert!(failure.output_truncated);
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
