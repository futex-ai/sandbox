//! E2B envd regular-file transfer operations.

use std::time::Duration;

use sandbox_interface::{
    BackendFileContent, BackendReadFileRequest, BackendWriteFileRequest, Error,
    FILE_TRANSFER_MAX_BYTES, FILE_TRANSFER_PATH_MAX_BYTES, Result,
};

use crate::process::{
    ProcessConnection, ProcessRegularFileRequest, ProcessRegularFileWriteRequest,
};

use super::{configured::E2bSandboxBackend, mapping};

pub(super) async fn read(
    backend: &E2bSandboxBackend,
    request: BackendReadFileRequest,
) -> Result<BackendFileContent> {
    validate_transfer_paths(&request.root, &request.path)?;
    let connection = mapping::read_only_connection(backend, &request.sandbox_provider_ref).await?;
    let content = backend
        .processes
        .read_regular_file(
            connection,
            ProcessRegularFileRequest {
                root: request.root,
                path: request.path,
                offset: request.offset,
                max_bytes: request.max_bytes,
                timeout: Duration::from_secs(300),
            },
        )
        .await?;
    if content.total_size > FILE_TRANSFER_MAX_BYTES as u64 {
        return Err(Error::FileTooLarge {
            limit: FILE_TRANSFER_MAX_BYTES,
        });
    }
    if content.bytes.len() > request.max_bytes {
        return Err(Error::ReadOnlyOutputTooLarge);
    }
    Ok(BackendFileContent {
        bytes: content.bytes,
        total_size: content.total_size,
    })
}

pub(super) async fn write(
    backend: &E2bSandboxBackend,
    request: BackendWriteFileRequest,
) -> Result<()> {
    validate_write_size(request.bytes.len())?;
    validate_transfer_paths(&request.root, &request.path)?;
    let connection = mapping::connection(backend, &request.sandbox_provider_ref).await?;
    write_to_connection(
        backend,
        connection,
        request.root,
        request.path,
        request.bytes,
    )
    .await
}

pub(super) async fn write_to_connection(
    backend: &E2bSandboxBackend,
    connection: ProcessConnection,
    root: String,
    path: String,
    bytes: Vec<u8>,
) -> Result<()> {
    validate_write_size(bytes.len())?;
    validate_transfer_paths(&root, &path)?;
    backend
        .processes
        .write_regular_file(
            connection,
            ProcessRegularFileWriteRequest { root, path, bytes },
        )
        .await
}

fn validate_transfer_paths(root: &str, path: &str) -> Result<()> {
    validate_transfer_path_length(root, "root")?;
    validate_transfer_path_length(path, "path")?;
    if root != "/" && !valid_path_parts(root.strip_prefix('/'), false) {
        return Err(Error::InvalidFilePath { field: "root" });
    }
    if !valid_path_parts(Some(path), true) {
        return Err(Error::InvalidFilePath { field: "path" });
    }
    Ok(())
}

fn validate_transfer_path_length(value: &str, field: &'static str) -> Result<()> {
    if value.len() > FILE_TRANSFER_PATH_MAX_BYTES {
        return Err(Error::TextTooLarge {
            field,
            limit: FILE_TRANSFER_PATH_MAX_BYTES,
        });
    }
    Ok(())
}

fn valid_path_parts(value: Option<&str>, relative: bool) -> bool {
    let Some(value) = value else {
        return false;
    };
    !value.is_empty()
        && (!relative || !value.starts_with('/'))
        && !value.contains('\0')
        && value
            .split('/')
            .all(|part| !matches!(part, "" | "." | ".."))
}

pub(super) fn validate_write_size(byte_count: usize) -> Result<()> {
    if byte_count > FILE_TRANSFER_MAX_BYTES {
        return Err(Error::FileTooLarge {
            limit: FILE_TRANSFER_MAX_BYTES,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "_tests_/file_transfer_tests.rs"]
mod file_transfer_tests;
