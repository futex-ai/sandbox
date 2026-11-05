//! Chunked verified upload and commit-or-revoke replacement reconciliation.

use std::time::Duration;

use sandbox_interface::{BackendWriteFileRequest, Error, FILE_TRANSFER_MAX_BYTES, Result};
use tokio::time::Instant;

use crate::{
    runtime::configured::BoatTrustedRuntime,
    trusted_runtime::{
        attempt::Attempt,
        manifest::{Manifest, digest},
    },
};

use super::validation;

pub(crate) async fn write(
    runtime: &BoatTrustedRuntime,
    request: BackendWriteFileRequest,
) -> Result<()> {
    validation::validate(&request.root, &request.path, request.bytes.len())?;
    let deadline = Instant::now() + Duration::from_secs(300);
    let expected = digest(&request.bytes);
    let manifest = Manifest::Write {
        root: request.root,
        path: request.path,
        expected_size: request.bytes.len(),
        expected_digest: expected,
        retain_fence: true,
    };
    let attempt = match Attempt::prepare(
        runtime.api.clone(),
        request.sandbox_provider_ref.as_str().to_owned(),
        runtime.config.workload_user().to_owned(),
        manifest,
        deadline,
    )
    .await
    {
        Ok(attempt) => attempt,
        Err(_) => return Err(runtime.unavailable()),
    };
    for (index, chunk) in request.bytes.chunks(4 * 1024 * 1024).enumerate() {
        let upload = attempt
            .api
            .upload(
                &attempt.sandbox,
                attempt.path(&format!("chunk-{index}")),
                chunk.to_vec(),
                attempt.remaining().unwrap_or_default(),
            )
            .await;
        if upload.is_err() {
            let _ = attempt.cleanup("reconcile-resolved").await;
            let _ = attempt.cleanup("clean").await;
            return Err(runtime.unavailable());
        }
    }
    let assembled = attempt.run("assemble").await;
    if !assembled
        .as_ref()
        .is_ok_and(BoatTrustedRuntime::helper_succeeded)
    {
        let _ = attempt.cleanup("reconcile-resolved").await;
        let _ = attempt.cleanup("clean").await;
        return Err(runtime.unavailable());
    }
    let written = attempt.run("write").await;
    if written
        .as_ref()
        .is_ok_and(BoatTrustedRuntime::helper_succeeded)
    {
        let _ = attempt.cleanup("clean").await;
        return Ok(());
    }
    let precommit = written
        .as_ref()
        .ok()
        .filter(|output| output.normal)
        .and_then(|output| output.exit_code)
        .filter(|code| matches!(code, 45..=48 | 50 | 51));
    let action = if precommit.is_some() {
        "reconcile-resolved"
    } else {
        "reconcile"
    };
    let cleanup = attempt.cleanup(action).await;
    if let Ok(output) = cleanup {
        if output.normal && output.exit_code == Some(51) {
            let _ = attempt.cleanup("clean").await;
            return Ok(());
        }
        if output.normal && output.exit_code == Some(0) {
            let _ = attempt.cleanup("clean").await;
            return Err(rejected(runtime, precommit));
        }
    }
    if precommit.is_some() {
        return Err(rejected(runtime, precommit));
    }
    Err(Error::FileWriteUnconfirmed)
}

fn rejected(runtime: &BoatTrustedRuntime, code: Option<i32>) -> Error {
    match code {
        Some(45) => Error::InvalidFilePath { field: "root" },
        Some(46) => Error::FileNotRegular,
        Some(47) => Error::FileTooLarge {
            limit: FILE_TRANSFER_MAX_BYTES,
        },
        Some(48) => Error::InvalidFilePath { field: "path" },
        _ => runtime.unavailable(),
    }
}
