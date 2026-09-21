//! E2B envd regular-file transfer operations.

use std::path::Path;

use sandbox_interface::{
    BackendFileContent, BackendReadFileRequest, BackendWriteFileRequest, Error,
    FILE_TRANSFER_MAX_BYTES, Result,
};

use crate::process::{ProcessConnection, ProcessRegularFileRequest};

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
    if bytes.len() > FILE_TRANSFER_MAX_BYTES {
        return Err(Error::FileTooLarge {
            limit: FILE_TRANSFER_MAX_BYTES,
        });
    }
    let validation = backend
        .processes
        .validate_file(connection.clone(), root, path, true)
        .await?;
    validate_target(&validation, true)?;
    backend
        .processes
        .upload_file(connection, validation.canonical_path, bytes)
        .await
}

fn validate_target(
    validation: &crate::process::ProcessFileValidation,
    allow_missing: bool,
) -> Result<()> {
    if !Path::new(&validation.canonical_path).starts_with(&validation.canonical_root) {
        return Err(Error::FileOutsideRoot);
    }
    if validation.symlink {
        return Err(Error::FileNotRegular);
    }
    if validation.exists && !validation.regular {
        return Err(Error::FileNotRegular);
    }
    if !validation.exists && !allow_missing {
        return Err(Error::FileNotRegular);
    }
    Ok(())
}

#[cfg(test)]
#[path = "_tests_/file_transfer_tests.rs"]
mod file_transfer_tests;
