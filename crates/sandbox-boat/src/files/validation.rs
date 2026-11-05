//! Provider-independent lexical validation before any Boat request.

use sandbox_interface::{Error, FILE_TRANSFER_MAX_BYTES, FILE_TRANSFER_PATH_MAX_BYTES, Result};

pub(super) fn validate(root: &str, path: &str, size: usize) -> Result<()> {
    for (field, value) in [("root", root), ("path", path)] {
        if value.len() > FILE_TRANSFER_PATH_MAX_BYTES {
            return Err(Error::TextTooLarge {
                field,
                limit: FILE_TRANSFER_PATH_MAX_BYTES,
            });
        }
    }
    let valid = |value: &str| {
        !value.is_empty()
            && !value.contains('\0')
            && value
                .split('/')
                .all(|part| !matches!(part, "" | "." | ".."))
    };
    if root != "/" && !root.strip_prefix('/').is_some_and(valid) {
        return Err(Error::InvalidFilePath { field: "root" });
    }
    if !valid(path) {
        return Err(Error::InvalidFilePath { field: "path" });
    }
    if size > FILE_TRANSFER_MAX_BYTES {
        return Err(Error::FileTooLarge {
            limit: FILE_TRANSFER_MAX_BYTES,
        });
    }
    Ok(())
}
