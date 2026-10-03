//! Process-stream fixture for E2B backend conformance coverage.

use std::collections::BTreeMap;

use futures_util::stream;
use sandbox_interface::{ProcessEventStream, ProcessStreamEvent, ProcessStreamOutcome, Result};

use crate::StreamProcessCommand;

pub(super) fn stream(command: StreamProcessCommand) -> Result<ProcessEventStream> {
    assert_eq!(command.command, "/bin/sh");
    assert_eq!(
        command.args,
        [
            "-c",
            "pwd; printf '%s' \"$SANDBOX_PROBE\"; printf '%s' 'stream-stderr' >&2"
        ]
    );
    assert_eq!(command.cwd.as_deref(), Some("/workspace"));
    assert_eq!(
        command.envs,
        BTreeMap::from([(
            "SANDBOX_PROBE".to_owned(),
            "stream-environment-map".to_owned()
        )])
    );
    Ok(Box::pin(stream::iter([
        ProcessStreamEvent::Started { pid: 13 },
        ProcessStreamEvent::Stdout(b"/workspace\nstream-environment-map".to_vec()),
        ProcessStreamEvent::Stderr(b"stream-stderr".to_vec()),
        ProcessStreamEvent::Exited {
            exit_code: 0,
            exited: true,
        },
        ProcessStreamEvent::Outcome(ProcessStreamOutcome::Completed),
    ])))
}
