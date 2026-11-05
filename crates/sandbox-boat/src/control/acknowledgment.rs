//! Exact response identity validation.

use crate::{
    error::{Error, Result},
    transport::request::{valid_sandbox_id, valid_snapshot_name},
};

use super::types::{BoatSandbox, BoatSavedSnapshot};

pub(super) fn sandbox(value: &BoatSandbox) -> Result<()> {
    if !valid_sandbox_id(&value.id) {
        return Err(Error::InvalidResponse);
    }
    Ok(())
}

pub(super) fn snapshot(value: &BoatSavedSnapshot, expected: &str) -> Result<()> {
    if value.name != expected
        || !valid_snapshot_name(&value.name)
        || !valid_sandbox_id(&value.source_sandbox_id)
    {
        return Err(Error::InvalidResponse);
    }
    Ok(())
}

pub(super) fn read_status(status: u16) -> Error {
    match status {
        401 | 403 => Error::Unauthorized,
        404 => Error::NotFound,
        400 | 402 | 409 => Error::InvalidRequest,
        _ => Error::Unavailable,
    }
}
