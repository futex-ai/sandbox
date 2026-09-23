//! Provider process duration validation.

use std::time::Duration;

use sandbox_interface::{Error, PROCESS_RUN_MAX_DEADLINE, PROCESS_STREAM_MAX_DEADLINE, Result};

pub(super) fn validate(field: &'static str, duration: Duration) -> Result<()> {
    if duration > PROCESS_RUN_MAX_DEADLINE {
        return Err(Error::InvalidSeconds {
            field,
            minimum: 0,
            maximum: PROCESS_RUN_MAX_DEADLINE.as_secs(),
        });
    }
    Ok(())
}

pub(super) fn validate_stream(field: &'static str, duration: Duration) -> Result<()> {
    if duration > PROCESS_STREAM_MAX_DEADLINE {
        return Err(Error::InvalidSeconds {
            field,
            minimum: 0,
            maximum: PROCESS_STREAM_MAX_DEADLINE.as_secs(),
        });
    }
    Ok(())
}
