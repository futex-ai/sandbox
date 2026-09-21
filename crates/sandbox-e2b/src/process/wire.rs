//! Typed envd Process Connect JSON wire DTOs.

use std::collections::BTreeMap;

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::error::{Error, Result};

use super::{
    selector::ProcessSelector,
    types::{ProcessCommand, ProcessPtyRequest},
};

const FILE_LIMIT_BLOCK_BYTES: usize = 1024;
const TERMINAL_WRAPPER: &str = concat!(
    "ulimit -S -f \"$SANDBOX_TERMINAL_LOG_BLOCKS\"; ",
    "exec /usr/bin/script -q -f --log-out \"$SANDBOX_TERMINAL_LOG_PATH\" ",
    "-c '/bin/bash -c \"ulimit -S -f unlimited; exec /bin/bash -il\"'"
);

pub(super) fn pty_start(request: ProcessPtyRequest) -> StartRequestWire {
    let envs = BTreeMap::from([
        (
            "SANDBOX_TERMINAL_LOG_BLOCKS".to_owned(),
            (request.log_limit / FILE_LIMIT_BLOCK_BYTES)
                .max(1)
                .to_string(),
        ),
        ("SANDBOX_TERMINAL_LOG_PATH".to_owned(), request.log_path),
        ("LANG".to_owned(), "C.UTF-8".to_owned()),
        ("LC_ALL".to_owned(), "C.UTF-8".to_owned()),
        ("TERM".to_owned(), "xterm-256color".to_owned()),
    ]);
    StartRequestWire {
        process: ProcessConfigWire {
            cmd: "/bin/bash".to_owned(),
            args: vec!["-lc".to_owned(), TERMINAL_WRAPPER.to_owned()],
            envs,
            cwd: request.cwd,
        },
        pty: Some(PtyWire {
            size: PtySizeWire {
                cols: 120,
                rows: 40,
            },
        }),
        tag: Some(request.tag),
        stdin: true,
    }
}

pub(super) fn command_start(command: ProcessCommand) -> StartRequestWire {
    process_start(command.command, command.args, command.cwd)
}

pub(super) fn argv_start(command: String, args: Vec<String>) -> StartRequestWire {
    process_start(command, args, None)
}

fn process_start(command: String, args: Vec<String>, cwd: Option<String>) -> StartRequestWire {
    StartRequestWire {
        process: ProcessConfigWire {
            cmd: command,
            args,
            envs: BTreeMap::new(),
            cwd,
        },
        pty: None,
        tag: None,
        stdin: false,
    }
}

pub(super) fn selector(pid: u32) -> SelectorRequestWire {
    SelectorRequestWire {
        process: SelectorWire::from(ProcessSelector::Pid(pid)),
    }
}

pub(super) fn send_input(selector: ProcessSelector, input: Vec<u8>) -> SendInputRequestWire {
    SendInputRequestWire {
        process: SelectorWire::from(selector),
        input: InputWire {
            pty: STANDARD.encode(input),
        },
    }
}

pub(super) fn signal(selector: ProcessSelector) -> SignalRequestWire {
    SignalRequestWire {
        process: SelectorWire::from(selector),
        signal: "SIGNAL_SIGKILL",
    }
}

pub(super) fn encode(value: &impl Serialize) -> Result<Vec<u8>> {
    match serde_json::to_vec(value) {
        Ok(bytes) => Ok(bytes),
        Err(source) => Err(Error::internal_with(source, "encode Connect request")),
    }
}

pub(super) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    match serde_json::from_slice(bytes) {
        Ok(value) => Ok(value),
        Err(source) => Err(Error::internal_with(
            source,
            "decode Connect unary response",
        )),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StartRequestWire {
    process: ProcessConfigWire,
    #[serde(skip_serializing_if = "Option::is_none")]
    pty: Option<PtyWire>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
    stdin: bool,
}

#[derive(Serialize)]
struct ProcessConfigWire {
    cmd: String,
    args: Vec<String>,
    envs: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
}

#[derive(Serialize)]
struct PtyWire {
    size: PtySizeWire,
}

#[derive(Serialize)]
struct PtySizeWire {
    cols: u32,
    rows: u32,
}

#[derive(Serialize)]
pub(super) struct SelectorRequestWire {
    process: SelectorWire,
}

#[derive(Serialize)]
#[serde(untagged)]
enum SelectorWire {
    Pid { pid: u32 },
    Tag { tag: String },
}

impl From<ProcessSelector> for SelectorWire {
    fn from(selector: ProcessSelector) -> Self {
        match selector {
            ProcessSelector::Pid(pid) => Self::Pid { pid },
            ProcessSelector::Tag(tag) => Self::Tag { tag },
        }
    }
}

#[derive(Serialize)]
pub(super) struct SendInputRequestWire {
    process: SelectorWire,
    input: InputWire,
}

#[derive(Serialize)]
struct InputWire {
    pty: String,
}

#[derive(Serialize)]
pub(super) struct SignalRequestWire {
    process: SelectorWire,
    signal: &'static str,
}

#[derive(Serialize)]
pub(super) struct EmptyWire {}

#[derive(Deserialize)]
pub(super) struct ListResponseWire {
    #[serde(default)]
    pub(super) processes: Vec<ProcessInfoWire>,
}

#[derive(Deserialize)]
pub(super) struct ProcessInfoWire {
    pub(super) pid: u32,
    pub(super) tag: Option<String>,
}

#[cfg(test)]
#[path = "_tests_/selector_tests.rs"]
mod selector_tests;
