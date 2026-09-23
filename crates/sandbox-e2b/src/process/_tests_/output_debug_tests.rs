//! Output formatting emits metadata while preserving raw captured data.

use std::fmt::Debug;

use crate::process::framing::{ProcessDataChannel, ProcessEvent};
use crate::process::types::{
    ProcessConnectOutput, ProcessFileChunk, ProcessRunOutput, ProcessSplitOutput,
};

#[test]
fn every_output_layer_keeps_bytes_out_of_debug() {
    let bytes = b"call-local-token\r\n\x1b[31m\xff".to_vec();
    let combined = ProcessRunOutput {
        bytes: bytes.clone(),
        exit_code: Some(7),
        exited: true,
        output_truncated: true,
    };
    let split = ProcessSplitOutput {
        stdout: bytes.clone(),
        stderr: bytes.clone(),
        exit_code: Some(7),
        exited: true,
        stdout_overflowed: false,
        stderr_overflowed: true,
    };
    let pty = ProcessConnectOutput {
        bytes: bytes.clone(),
        exit_code: None,
        exited: false,
    };
    let transcript = ProcessFileChunk {
        bytes: bytes.clone(),
        total_size: 128,
    };
    let event = ProcessEvent::Data {
        channel: ProcessDataChannel::Pty,
        bytes: bytes.clone(),
    };

    for output in [&combined as &dyn Debug, &split, &pty, &transcript, &event] {
        for formatted in [format!("{output:?}"), format!("{output:#?}")] {
            assert!(!formatted.contains("bytes: ["));
            assert!(!formatted.contains("stdout:"));
            assert!(!formatted.contains("stderr:"));
            assert!(formatted.contains("bytes:"));
        }
    }
    assert_eq!(
        format!("{combined:?}"),
        format!(
            "ProcessRunOutput {{ output_bytes: {}, exit_code: Some(7), exited: true, output_truncated: true }}",
            bytes.len()
        )
    );
    assert_eq!(combined.bytes, bytes);
    assert_eq!(split.stdout, bytes);
    assert_eq!(split.stderr, bytes);
    assert_eq!(pty.bytes, bytes);
    assert_eq!(transcript.bytes, bytes);
    assert_eq!(
        event,
        ProcessEvent::Data {
            channel: ProcessDataChannel::Pty,
            bytes
        }
    );
}
