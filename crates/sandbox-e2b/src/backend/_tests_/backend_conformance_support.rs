//! Process-stream fixture for E2B backend conformance coverage.

use futures_util::stream;
use sandbox_interface::{ProcessEventStream, ProcessStreamEvent, ProcessStreamOutcome, Result};

use crate::StreamProcessCommand;

pub(super) fn stream(command: StreamProcessCommand) -> Result<ProcessEventStream> {
    assert_eq!(command.command, "/bin/sh");
    assert_eq!(
        command.args,
        [
            "-c",
            "printf '%s' 'stream-stdout'; printf '%s' 'stream-stderr' >&2"
        ]
    );
    Ok(Box::pin(stream::iter([
        ProcessStreamEvent::Started { pid: 13 },
        ProcessStreamEvent::Stdout(b"stream-stdout".to_vec()),
        ProcessStreamEvent::Stderr(b"stream-stderr".to_vec()),
        ProcessStreamEvent::Exited { exit_code: 0 },
        ProcessStreamEvent::Outcome(ProcessStreamOutcome::Completed),
    ])))
}
