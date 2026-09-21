//! In-memory file behavior for the alternate conformance backend.

use std::{collections::HashMap, sync::Mutex};

use sandbox_interface::{
    BackendFileContent, BackendReadFileRequest, BackendWriteFileRequest, Result,
};

pub(super) fn read(
    files: &Mutex<HashMap<String, Vec<u8>>>,
    request: BackendReadFileRequest,
) -> Result<BackendFileContent> {
    let bytes = files
        .lock()
        .expect("file lock")
        .get(&request.path)
        .cloned()
        .unwrap_or_default();
    let total_size = u64::try_from(bytes.len()).expect("file size");
    let offset = usize::try_from(request.offset).expect("file offset");
    let bytes = bytes
        .get(offset..)
        .unwrap_or_default()
        .iter()
        .copied()
        .take(request.max_bytes)
        .collect();
    Ok(BackendFileContent { bytes, total_size })
}

pub(super) fn write(
    files: &Mutex<HashMap<String, Vec<u8>>>,
    request: BackendWriteFileRequest,
) -> Result<()> {
    files
        .lock()
        .expect("file lock")
        .insert(request.path, request.bytes);
    Ok(())
}
