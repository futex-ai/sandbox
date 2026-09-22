//! Fake envd process boundary for shared E2B conformance coverage.

use std::sync::{Arc, Mutex};

use sandbox_interface::Result;
use unimock::{MockFn, Unimock, matching};

use crate::{
    ProcessFileChunk, ProcessInfo, ProcessRegularFileRequest, ProcessRegularFileWriteRequest,
    ProcessRunOutput, ProcessSelector, ProcessSplitOutput, ProcessTransportMock,
};

pub(super) fn transport() -> Unimock {
    let active_terminal = Arc::new(Mutex::new(None));
    Unimock::new((
        ProcessTransportMock::run
            .each_call(matching!(_, _))
            .answers(&|_, _, command| {
                let setup_fails = command
                    .args
                    .iter()
                    .any(|argument| argument.contains("exit 7"));
                let measures_image = command
                    .args
                    .iter()
                    .any(|argument| argument.contains("du -sbx"));
                let bytes = if measures_image {
                    b"__SANDBOX_IMAGE_SIZE__=8192\n".to_vec()
                } else {
                    Vec::new()
                };
                Ok(ProcessRunOutput {
                    bytes,
                    exit_code: Some(if setup_fails { 7 } else { 0 }),
                    exited: true,
                    output_truncated: false,
                })
            }),
        ProcessTransportMock::list
            .each_call(matching!(_))
            .answers_arc({
                let active_terminal = active_terminal.clone();
                Arc::new(move |_, _| {
                    Ok(active_terminal
                        .lock()
                        .expect("active terminal lock")
                        .clone()
                        .into_iter()
                        .collect())
                })
            }),
        ProcessTransportMock::start_pty
            .each_call(matching!(_, _))
            .answers_arc({
                let active_terminal = active_terminal.clone();
                Arc::new(move |_, _, request| {
                    let process = ProcessInfo {
                        pid: 9,
                        tag: Some(request.tag.clone()),
                    };
                    *active_terminal.lock().expect("active terminal lock") = Some(process.clone());
                    Ok(process)
                })
            }),
        ProcessTransportMock::send_input
            .each_call(matching!(_, _, _))
            .answers(&|_, _, selector, _| {
                assert!(matches!(selector, ProcessSelector::Tag(_)));
                Ok(())
            }),
        ProcessTransportMock::read_regular_file
            .each_call(matching!(_, _))
            .answers(&|_, _, request: ProcessRegularFileRequest| {
                if request.root == "/workspace" {
                    assert_eq!(request.path, "conformance.txt");
                    Ok(ProcessFileChunk {
                        bytes: b"file-transfer".to_vec(),
                        total_size: 13,
                    })
                } else {
                    assert_eq!(request.root, "/var/lib/sandbox-e2b/terminals");
                    assert!(request.path.ends_with(".log"));
                    assert_eq!(request.offset, 0);
                    assert_eq!(request.max_bytes, 4096);
                    Ok(ProcessFileChunk {
                        bytes: b"conformance".to_vec(),
                        total_size: 11,
                    })
                }
            }),
        ProcessTransportMock::write_regular_file
            .each_call(matching!(_, _))
            .answers(&|_, _, request: ProcessRegularFileWriteRequest| {
                if request.path == "conformance.txt" {
                    assert_eq!(request.root, "/workspace");
                    assert_eq!(request.bytes, b"file-transfer");
                } else {
                    assert_eq!(request.bytes, b"input");
                }
                Ok(())
            }),
        ProcessTransportMock::run_split
            .each_call(matching!(_, _))
            .answers(&|_, _, command| split_output(&command.command, &command.args)),
        ProcessTransportMock::kill
            .each_call(matching!(_, _))
            .answers(&|_, _, selector| {
                assert!(matches!(selector, ProcessSelector::Tag(_)));
                Ok(())
            }),
    ))
}

fn split_output(command: &str, args: &[String]) -> Result<ProcessSplitOutput> {
    assert_eq!(command, "/bin/sh");
    assert_eq!(args.first().map(String::as_str), Some("-c"));
    match args.get(1).map(String::as_str) {
        Some("printf '%s' 'argv-direct'; printf '%s' 'separate-stderr' >&2") => {
            Ok(ProcessSplitOutput {
                stdout: b"argv-direct".to_vec(),
                stderr: b"separate-stderr".to_vec(),
                exit_code: Some(0),
                exited: true,
                ..ProcessSplitOutput::default()
            })
        }
        Some(script) if script.ends_with("https://example.com/") => Ok(success(0)),
        Some(script) if script.ends_with("https://www.google.com/") => Ok(success(28)),
        script => panic!("unexpected conformance process script: {script:?}"),
    }
}

fn success(exit_code: i32) -> ProcessSplitOutput {
    ProcessSplitOutput {
        exit_code: Some(exit_code),
        exited: true,
        ..ProcessSplitOutput::default()
    }
}
