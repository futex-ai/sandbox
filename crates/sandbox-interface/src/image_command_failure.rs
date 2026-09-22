//! Safe command-failure diagnostics for platform image realization.

use std::cmp::Reverse;

use serde::{Deserialize, Serialize};

use crate::IMAGE_COMMAND_OUTPUT_MAX_BYTES;

const REDACTION: &[u8] = b"[REDACTED]";

/// Provider-neutral failure facts from one user-authored image command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ImageCommandFailure {
    /// Observed process exit code, when the backend received one.
    pub exit_code: Option<i32>,
    /// Safe bounded tail of combined standard output and standard error.
    pub output: Option<String>,
    /// Whether bytes were omitted from the returned diagnostic window.
    pub output_truncated: bool,
}

impl ImageCommandFailure {
    /// Normalizes captured process bytes into the provider-neutral diagnostic.
    ///
    /// Backends pass any sensitive values available at this boundary. Empty
    /// values are ignored so they cannot rewrite every byte boundary. Truncated
    /// leading suffixes are redacted before complete sensitive values, output
    /// normalization, and final tail bounding.
    #[must_use]
    pub fn from_captured_output(
        bytes: &[u8],
        exit_code: Option<i32>,
        capture_truncated: bool,
        sensitive_values: &[String],
    ) -> Self {
        let redacted = redact_truncated_prefix(bytes.to_vec(), capture_truncated, sensitive_values);
        let redacted = redact_complete_values(&redacted, sensitive_values);
        let stripped = strip_ansi_escapes::strip(redacted);
        let stripped = redact_truncated_prefix(stripped, capture_truncated, sensitive_values);
        let mut output = String::from_utf8_lossy(&stripped).replace("\r\n", "\n");
        output.retain(|character| matches!(character, '\n' | '\t') || !character.is_control());
        let mut redactions = sensitive_values
            .iter()
            .map(String::as_str)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        redactions.sort_unstable_by_key(|value| Reverse(value.len()));
        for value in redactions {
            output = output.replace(value, "[REDACTED]");
        }
        let (output, normalized_truncated) = bounded_tail(output);
        Self {
            exit_code,
            output: (!output.is_empty()).then_some(output),
            output_truncated: capture_truncated || normalized_truncated,
        }
    }
}

fn redact_complete_values(bytes: &[u8], sensitive_values: &[String]) -> Vec<u8> {
    let mut redactions = sensitive_values
        .iter()
        .map(String::as_bytes)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    redactions.sort_unstable_by_key(|value| Reverse(value.len()));
    let mut output = Vec::with_capacity(bytes.len());
    let mut cursor = 0;
    while cursor < bytes.len() {
        let matched = redactions
            .iter()
            .find_map(|value| bytes[cursor..].starts_with(value).then_some(value.len()));
        if let Some(length) = matched {
            output.extend_from_slice(REDACTION);
            cursor += length;
        } else {
            output.push(bytes[cursor]);
            cursor += 1;
        }
    }
    output
}

fn redact_truncated_prefix(
    bytes: Vec<u8>,
    capture_truncated: bool,
    sensitive_values: &[String],
) -> Vec<u8> {
    if !capture_truncated || bytes.is_empty() {
        return bytes;
    }
    let mut matched = 0;
    for value in sensitive_values.iter().filter(|value| !value.is_empty()) {
        let sensitive = value.as_bytes();
        let maximum = sensitive.len().min(bytes.len());
        for length in (1..=maximum).rev() {
            if bytes.starts_with(&sensitive[sensitive.len() - length..]) {
                matched = matched.max(length);
                break;
            }
        }
    }
    if matched == 0 {
        return bytes;
    }
    let mut redacted = REDACTION.to_vec();
    redacted.extend_from_slice(&bytes[matched..]);
    redacted
}

fn bounded_tail(output: String) -> (String, bool) {
    if output.len() <= IMAGE_COMMAND_OUTPUT_MAX_BYTES {
        return (output, false);
    }
    let mut start = output.len() - IMAGE_COMMAND_OUTPUT_MAX_BYTES;
    while !output.is_char_boundary(start) {
        start += 1;
    }
    (output[start..].to_owned(), true)
}

#[cfg(test)]
#[path = "_tests_/image_command_failure_tests.rs"]
mod image_command_failure_tests;
