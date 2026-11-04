//! Isolated fake account resource state and captured requests.

use std::collections::{BTreeMap, VecDeque};

use serde_json::{Value, json};

use super::wire::{CapturedRequest, Reply};

pub const CAPTURE_SOURCE: &str = "bx_abcdefgh";

pub struct Override {
    pub method: String,
    pub path: String,
    pub reply: Reply,
}

pub struct State {
    pub requests: Vec<CapturedRequest>,
    pub sandboxes: BTreeMap<String, Value>,
    pub snapshots: BTreeMap<String, Value>,
    pub overrides: VecDeque<Override>,
    pub machine_types: Value,
    pub page_size: usize,
    pub snapshot_status: String,
    pub deletion_statuses: VecDeque<String>,
    pub create_reply: Option<Reply>,
    pub save_reply: Option<Reply>,
    next_id: u32,
    pub runtime: super::runtime_state::RuntimeState,
}

impl Default for State {
    fn default() -> Self {
        Self {
            requests: Vec::new(),
            snapshots: BTreeMap::new(),
            overrides: VecDeque::new(),
            sandboxes: BTreeMap::from([(
                CAPTURE_SOURCE.to_owned(),
                sandbox(CAPTURE_SOURCE, json!({}), true),
            )]),
            machine_types: json!([
                {"type":"small","available":true,"reason":null},
                {"type":"default","available":true,"reason":null},
                {"type":"large","available":true,"reason":null},
                {"type":"xlarge","available":false,"reason":"plan required"}
            ]),
            page_size: 200,
            snapshot_status: "ready".to_owned(),
            deletion_statuses: VecDeque::new(),
            create_reply: None,
            save_reply: None,
            next_id: 0,
            runtime: Default::default(),
        }
    }
}

impl State {
    pub fn allocate_id(&mut self) -> String {
        let digit = b"23456789abcdefghjkmnpqrstuvwxyz"[self.next_id as usize];
        self.next_id += 1;
        format!("bx_2222222{}", char::from(digit))
    }
}

pub fn sandbox(id: &str, labels: Value, snapshots: bool) -> Value {
    json!({"id":id,"state":"ready","labels":labels,"type":"default","ttlSeconds":21600,
        "snapshots":snapshots,"monitoring":false})
}

pub fn error(status: u16, code: &str) -> Reply {
    Reply::json(
        status,
        json!({"ok":false,"type":"sandbox.error","status":status,"code":code,
        "message":"fixture rejection","error":{"status":status,"code":code,"message":"fixture rejection"},"requestId":"fixture"}),
    )
}
