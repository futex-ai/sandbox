//! Stateful native staging and detached-process fixtures.

use std::{collections::BTreeMap, sync::Arc};

use serde_json::Value;
use tokio::sync::Notify;

#[derive(Clone, Copy, Default, Debug)]
pub enum Fault {
    #[default]
    None,
    MissingSuccess,
    OffsetGap,
    WrongAttempt,
    AfterFinal,
    SupervisorSignal,
    MalformedThenEnd,
    Reconnect,
    Backpressure,
    HoldAfterPid,
}

pub struct Process {
    pub attempt: String,
    pub release: Arc<Notify>,
    pub ack: Arc<Notify>,
    pub connections: usize,
    pub remaining: Vec<u8>,
    pub offset: u64,
}

pub struct RuntimeState {
    pub bundles: BTreeMap<String, Value>,
    pub staging: BTreeMap<String, Vec<u8>>,
    pub files: BTreeMap<String, Vec<u8>>,
    pub processes: BTreeMap<u64, Process>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub code: i32,
    pub start_pid: u32,
    pub exited: bool,
    pub fault: Fault,
    pub finish: Option<Arc<Notify>>,
    pub opened: Arc<Notify>,
    pub terminated: Arc<Notify>,
    pub cleaned: Arc<Notify>,
    pub actions: Vec<String>,
    pub manifests: Vec<Value>,
    pub write_failure: Option<i32>,
    pub cleanup_failure: bool,
    pub upload_tamper: bool,
    pub read_tamper: bool,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            bundles: BTreeMap::new(),
            staging: BTreeMap::new(),
            files: BTreeMap::new(),
            processes: BTreeMap::new(),
            stdout: b"stdout".to_vec(),
            stderr: b"stderr".to_vec(),
            code: 0,
            start_pid: 100,
            exited: true,
            fault: Fault::None,
            finish: None,
            opened: Arc::new(Notify::new()),
            terminated: Arc::new(Notify::new()),
            cleaned: Arc::new(Notify::new()),
            actions: Vec::new(),
            manifests: Vec::new(),
            write_failure: None,
            cleanup_failure: false,
            upload_tamper: false,
            read_tamper: false,
        }
    }
}
