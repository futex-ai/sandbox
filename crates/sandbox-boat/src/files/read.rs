//! Descriptor-bound ranged reads exported only through protected staging chunks.

use std::time::Duration;

use sandbox_interface::{
    BackendFileContent, BackendReadFileRequest, Error, FILE_TRANSFER_MAX_BYTES, ResourceKind,
    Result,
};
use tokio::time::Instant;

use crate::{
    runtime::{client::decode, configured::BoatTrustedRuntime},
    trusted_runtime::{
        attempt::Attempt,
        manifest::{Manifest, ReadResult, digest},
    },
};

use super::validation;

pub(crate) async fn read(
    runtime: &BoatTrustedRuntime,
    request: BackendReadFileRequest,
) -> Result<BackendFileContent> {
    validation::validate(&request.root, &request.path, request.max_bytes)?;
    let deadline = Instant::now() + Duration::from_secs(300);
    let maximum = request.max_bytes;
    let offset = request.offset;
    let manifest = Manifest::Read {
        root: request.root,
        path: request.path,
        offset,
        max_bytes: maximum,
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
    let result = read_chunks(runtime, &attempt, maximum, offset).await;
    let _ = attempt.cleanup("clean").await;
    result
}

async fn read_chunks(
    runtime: &BoatTrustedRuntime,
    attempt: &Attempt,
    maximum: usize,
    offset: u64,
) -> Result<BackendFileContent> {
    let output = match attempt.run("read").await {
        Ok(output) => output,
        Err(_) => return Err(runtime.unavailable()),
    };
    if !output.normal {
        return Err(runtime.unavailable());
    }
    match output.exit_code {
        Some(0) => {}
        Some(44) => {
            return Err(Error::NotFound {
                resource: ResourceKind::File,
            });
        }
        Some(45) => return Err(Error::InvalidFilePath { field: "root" }),
        Some(46) => return Err(Error::FileNotRegular),
        Some(47) => {
            return Err(Error::FileTooLarge {
                limit: FILE_TRANSFER_MAX_BYTES,
            });
        }
        Some(48) => return Err(Error::InvalidFilePath { field: "path" }),
        _ => return Err(runtime.unavailable()),
    }
    let result: ReadResult = match decode(output.stdout.as_bytes()) {
        Ok(value) => value,
        Err(_) => return Err(runtime.unavailable()),
    };
    let selected = result.total_size.saturating_sub(offset).min(maximum as u64) as usize;
    if result.version != 1
        || result.attempt != attempt.id
        || result.total_size > FILE_TRANSFER_MAX_BYTES as u64
        || result.sizes.len() != selected.div_ceil(4 * 1024 * 1024)
    {
        return Err(runtime.unavailable());
    }
    let mut bytes = Vec::with_capacity(selected);
    for (index, length) in result.sizes.into_iter().enumerate() {
        if length != (selected - bytes.len()).min(4 * 1024 * 1024) {
            return Err(runtime.unavailable());
        }
        let chunk = match attempt
            .api
            .download(
                &attempt.sandbox,
                attempt.path(&format!("chunk-{index}")),
                attempt.remaining().unwrap_or_default(),
            )
            .await
        {
            Ok(chunk) => chunk,
            Err(_) => return Err(runtime.unavailable()),
        };
        if chunk.len() != length {
            return Err(runtime.unavailable());
        }
        bytes.extend(chunk);
    }
    if bytes.len() != selected || digest(&bytes) != result.digest {
        return Err(runtime.unavailable());
    }
    Ok(BackendFileContent {
        bytes,
        total_size: result.total_size,
    })
}
