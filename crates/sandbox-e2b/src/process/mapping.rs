//! Process-transport error mapping.

use sandbox_interface::{Error as DomainError, ResourceKind};

use crate::error::Error;

pub(super) fn map_error(error: Error, ambiguous_input: bool, backend_id: &str) -> DomainError {
    match error {
        Error::NotFound => DomainError::NotFound {
            resource: ResourceKind::Terminal,
        },
        Error::DeliveryAmbiguous if ambiguous_input => DomainError::DeliveryUnknown,
        Error::Unavailable | Error::DeliveryAmbiguous => DomainError::BackendUnavailable {
            backend_id: backend_id.to_owned(),
        },
        error => DomainError::internal_with(error, "E2B process transport"),
    }
}

pub(super) fn map_result<T>(
    result: crate::error::Result<T>,
    ambiguous_input: bool,
    backend_id: &str,
) -> Result<T, DomainError> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => Err(map_error(error, ambiguous_input, backend_id)),
    }
}

/// Maps envd file-transfer absence to the provider-neutral file resource.
pub(super) fn map_file_result<T>(
    result: crate::error::Result<T>,
    backend_id: &str,
) -> Result<T, DomainError> {
    match result {
        Ok(value) => Ok(value),
        Err(Error::NotFound) => Err(DomainError::NotFound {
            resource: ResourceKind::File,
        }),
        Err(error) => Err(map_error(error, false, backend_id)),
    }
}
