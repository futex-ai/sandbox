//! Regular-file transfer probe with explicit file capability.

use crate::{
    BackendReadFileRequest, BackendWriteFileRequest, Error, ProviderRef, Result, SandboxFiles,
};

/// Proves exact regular-file replacement and bounded read content.
///
/// The caller owns the sandbox and its `/workspace` fixture directory.
pub async fn exercise_files(
    backend: &dyn SandboxFiles,
    sandbox_provider_ref: ProviderRef,
) -> Result<()> {
    backend
        .write_file(BackendWriteFileRequest {
            sandbox_provider_ref: sandbox_provider_ref.clone(),
            root: "/workspace".to_owned(),
            path: "conformance.txt".to_owned(),
            bytes: b"file-transfer".to_vec(),
        })
        .await?;
    let file = backend
        .read_file(BackendReadFileRequest {
            sandbox_provider_ref: sandbox_provider_ref.clone(),
            root: "/workspace".to_owned(),
            path: "conformance.txt".to_owned(),
            offset: 0,
            max_bytes: 4096,
        })
        .await?;
    if file.bytes != b"file-transfer" || file.total_size != 13 {
        return Err(Error::internal_message(
            "backend file transfer changed regular-file content",
        ));
    }
    Ok(())
}
