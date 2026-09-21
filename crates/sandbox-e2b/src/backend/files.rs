//! E2B envd regular-file transfer operations.

use sandbox_interface::{
    BackendFileContent, BackendReadFileRequest, BackendWriteFileRequest, Error,
    FILE_TRANSFER_MAX_BYTES, Result,
};

use crate::process::{
    ProcessConnection, ProcessRegularFileRequest, ProcessRegularFileWriteRequest,
};

use super::{configured::E2bSandboxBackend, mapping};

pub(super) async fn read(
    backend: &E2bSandboxBackend,
    request: BackendReadFileRequest,
) -> Result<BackendFileContent> {
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
    backend
        .processes
        .write_regular_file(
            connection,
            ProcessRegularFileWriteRequest { root, path, bytes },
        )
        .await
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
