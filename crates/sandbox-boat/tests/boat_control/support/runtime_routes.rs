//! Documented command and native file envelopes with private staging semantics.

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use url::Url;

use super::{
    runtime_frames,
    runtime_state::{Process, RuntimeState},
    state::error,
    wire::{CapturedRequest, Reply},
};

pub(super) fn handle(
    state: &mut RuntimeState,
    request: &CapturedRequest,
    url: &Url,
) -> Option<Reply> {
    let path = url.path();
    if path.ends_with("/commands") && request.method == "POST" {
        let command = request.body["command"].as_str().unwrap();
        let fields = command.split(' ').collect::<Vec<_>>();
        let action = *fields.last().unwrap();
        if request.body["detached"] == true {
            let attempt = fields[fields.len() - 3].to_owned();
            let process = state.processes.len() as u64 + 1;
            state.processes.insert(
                process,
                Process {
                    attempt,
                    release: Default::default(),
                    ack: Default::default(),
                    connections: 0,
                    remaining: Vec::new(),
                    offset: 0,
                },
            );
            return Some(Reply::json(
                200,
                json!({"ok":true,"type":"command.started","success":true,"processId":process,"pid":state.start_pid}),
            ));
        }
        if action.len() == 32 {
            return Some(finished(0, ""));
        }
        let attempt = fields[fields.len() - 3];
        state.actions.push(action.to_owned());
        let reply = helper(state, attempt, action);
        return Some(reply);
    }
    if path.contains("/commands/") && request.method == "GET" {
        return Some(runtime_frames::attach(state, url));
    }
    if path.contains("/commands/") && request.method == "DELETE" {
        state.actions.push("stop".to_owned());
        state.terminated.notify_one();
        return Some(Reply::json(200, json!({"ok":true})));
    }
    if path.ends_with("/files") && request.method == "PUT" {
        let path = request.body["path"].as_str().unwrap();
        let mut bytes = STANDARD
            .decode(request.body["content"].as_str().unwrap())
            .unwrap();
        assert!(bytes.len() <= 4 * 1024 * 1024);
        let size = bytes.len();
        if path.ends_with("bundle.json") {
            let bundle: Value = serde_json::from_slice(&bytes).unwrap();
            state.manifests.push(bundle["manifest"].clone());
            state
                .bundles
                .insert(bundle["attempt"].as_str().unwrap().to_owned(), bundle);
        } else if state.upload_tamper && !bytes.is_empty() {
            bytes[0] ^= 1;
        }
        state.staging.insert(path.to_owned(), bytes);
        return Some(Reply::json(
            200,
            json!({"ok":true,"type":"file.written","success":true,
            "path":path,"encoding":"base64","size":size}),
        ));
    }
    if path.ends_with("/files") && request.method == "GET" {
        let path = url
            .query_pairs()
            .find(|(k, _)| k == "path")
            .unwrap()
            .1
            .to_string();
        let mut bytes = state.staging[&path].clone();
        if state.read_tamper && !bytes.is_empty() {
            bytes[0] ^= 1;
        }
        return Some(Reply::json(
            200,
            json!({"ok":true,"type":"file.read","success":true,
            "path":path,"encoding":"base64","size":bytes.len(),"content":STANDARD.encode(bytes)}),
        ));
    }
    None
}

fn helper(state: &mut RuntimeState, attempt: &str, action: &str) -> Reply {
    if matches!(action, "release" | "ack" | "terminate") {
        for process in state.processes.values().filter(|p| p.attempt == attempt) {
            if action == "release" {
                process.release.notify_one();
            }
            if action == "ack" {
                process.ack.notify_one();
            }
        }
        if action == "terminate" {
            state.terminated.notify_one();
        }
        return finished(0, "");
    }
    if action == "clean" {
        state.staging.retain(|path, _| !path.contains(attempt));
        state.cleaned.notify_one();
        return finished(0, "");
    }
    if state.cleanup_failure && action.starts_with("reconcile") {
        return finished(52, "");
    }
    let manifest = state.bundles[attempt]["manifest"].clone();
    let file = format!(
        "{}/{}",
        manifest["root"].as_str().unwrap(),
        manifest["path"].as_str().unwrap()
    );
    match action {
        "read" => {
            let Some(bytes) = state.files.get(&file).cloned() else {
                return finished(44, "");
            };
            let offset = manifest["offset"].as_u64().unwrap().min(bytes.len() as u64) as usize;
            let maximum = manifest["max_bytes"].as_u64().unwrap() as usize;
            let selected = &bytes[offset..offset + maximum.min(bytes.len() - offset)];
            let mut sizes = Vec::new();
            for (index, chunk) in selected.chunks(4 * 1024 * 1024).enumerate() {
                state
                    .staging
                    .insert(stage(attempt, &format!("chunk-{index}")), chunk.to_vec());
                sizes.push(chunk.len());
            }
            finished(
                0,
                &json!({"version":1,"attempt":attempt,"total_size":bytes.len(),
                "sizes":sizes,"digest":digest(selected)})
                .to_string(),
            )
        }
        "assemble" => {
            let mut bytes = Vec::new();
            let mut index = 0;
            while let Some(chunk) = state
                .staging
                .get(&stage(attempt, &format!("chunk-{index}")))
            {
                bytes.extend(chunk);
                index += 1;
            }
            if digest(&bytes) != manifest["expected_digest"]
                || bytes.len() as u64 != manifest["expected_size"]
            {
                return finished(51, "");
            }
            state.staging.insert(stage(attempt, "payload"), bytes);
            finished(0, "")
        }
        "write" => {
            if let Some(code) = state.write_failure {
                return finished(code, "");
            }
            let bytes = state.staging[&stage(attempt, "payload")].clone();
            state.files.insert(file, bytes);
            finished(0, "")
        }
        "reconcile" | "reconcile-resolved" => {
            let committed = state
                .files
                .get(&file)
                .is_some_and(|b| digest(b) == manifest["expected_digest"]);
            finished(if committed { 51 } else { 0 }, "")
        }
        _ => error(400, "unknown_runtime_action"),
    }
}

pub fn finished(code: i32, stdout: &str) -> Reply {
    Reply::json(
        200,
        json!({"ok":true,"type":"command.finished","success":code==0,
        "exitCode":code,"stdout":stdout,"stderr":"","timedOut":false,"signal":null}),
    )
}
pub fn stage(attempt: &str, name: &str) -> String {
    format!("/home/user/.sandbox-boat/{attempt}/{name}")
}
pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
