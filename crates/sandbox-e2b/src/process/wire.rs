//! Typed envd Process Connect JSON wire DTOs.

use std::collections::BTreeMap;

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    error::{Error, Result},
    trusted_python,
};

use super::{
    provider_pid::ProviderPid,
    selector::ProcessSelector,
    types::{ProcessCommand, ProcessPtyRequest},
};

const TERMINAL_WRAPPER: &str = include_str!("helpers/terminal_transcript.py");

pub(super) fn pty_start(request: ProcessPtyRequest) -> StartRequestWire {
    let envs = BTreeMap::from([
        ("LANG".to_owned(), "C.UTF-8".to_owned()),
        ("LC_ALL".to_owned(), "C.UTF-8".to_owned()),
        ("TERM".to_owned(), "xterm-256color".to_owned()),
    ]);
    StartRequestWire {
        process: ProcessConfigWire {
            cmd: trusted_python::EXECUTABLE.to_owned(),
            args: trusted_python::command_args(
                TERMINAL_WRAPPER,
                [
                    request.log_path,
                    request.log_limit.to_string(),
                    request.workload_user,
                ],
            ),
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
    process_start(command.command, command.args, command.cwd, command.envs)
}

pub(super) fn argv_start(
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
    envs: BTreeMap<String, String>,
) -> StartRequestWire {
    process_start(command, args, cwd, envs)
}

fn process_start(
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
    envs: BTreeMap<String, String>,
) -> StartRequestWire {
    StartRequestWire {
        process: ProcessConfigWire {
            cmd: command,
            args,
            envs,
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

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmptyWire {}

#[derive(Deserialize)]
pub(super) struct ListResponseWire {
    #[serde(default)]
    pub(super) processes: Vec<ProcessInfoWire>,
}

#[derive(Deserialize)]
pub(super) struct ProcessInfoWire {
    pub(super) pid: ProviderPid,
    pub(super) tag: Option<String>,
}

#[cfg(test)]
#[path = "_tests_/selector_tests.rs"]
mod selector_tests;

#[cfg(test)]
#[path = "_tests_/wire_tests.rs"]
mod wire_tests;
