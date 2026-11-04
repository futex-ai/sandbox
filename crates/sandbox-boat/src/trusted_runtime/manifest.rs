//! Typed private manifests and exact trusted helper source identities.

use std::collections::BTreeMap;

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Immutable private command or file operation data.
#[derive(Clone, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub(crate) enum Manifest {
    Process {
        command: String,
        args: Vec<String>,
        cwd: Option<String>,
        envs: BTreeMap<String, String>,
        timeout_ms: u64,
    },
    Read {
        root: String,
        path: String,
        offset: u64,
        max_bytes: usize,
    },
    Write {
        root: String,
        path: String,
        expected_size: usize,
        expected_digest: String,
        retain_fence: bool,
    },
}

#[derive(Serialize)]
pub(crate) struct Bundle {
    pub(crate) version: u8,
    pub(crate) attempt: String,
    pub(crate) workload_user: String,
    pub(crate) workspace: &'static str,
    pub(crate) manifest: Manifest,
    pub(crate) sources: BTreeMap<&'static str, String>,
}

impl Bundle {
    pub(crate) fn new(attempt: String, workload_user: String, manifest: Manifest) -> Self {
        Self {
            version: 1,
            attempt,
            workload_user,
            workspace: "/workspace",
            manifest,
            sources: BTreeMap::from([
                ("common", STANDARD.encode(include_str!("helpers/common.py"))),
                (
                    "process",
                    STANDARD.encode(include_str!("helpers/process.py")),
                ),
                ("files", STANDARD.encode(include_str!("helpers/files.py"))),
                (
                    "writer",
                    STANDARD.encode(sandbox_runtime_assets::REGULAR_FILE_WRITE),
                ),
                (
                    "cleanup",
                    STANDARD.encode(sandbox_runtime_assets::REGULAR_FILE_CLEANUP),
                ),
            ]),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadResult {
    pub(crate) version: u8,
    pub(crate) attempt: String,
    pub(crate) total_size: u64,
    pub(crate) sizes: Vec<usize>,
    pub(crate) digest: String,
}

pub(crate) fn digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut value = String::with_capacity(64);
    for byte in digest {
        value.push(char::from(HEX[usize::from(byte >> 4)]));
        value.push(char::from(HEX[usize::from(byte & 15)]));
    }
    value
}
