//! Explicit control response and timeout budgets.

use std::time::Duration;

use crate::error::{Error, Result};

/// Bounded HTTP settings for Boat control requests.
#[derive(Clone, Copy, Debug)]
pub struct BoatHttpSettings {
    /// Maximum connection setup duration.
    pub connect_timeout: Duration,
    /// Maximum time without the next response chunk.
    pub read_timeout: Duration,
    /// Absolute budget for one request, including its response body.
    pub request_timeout: Duration,
    /// Maximum response bytes, including error responses.
    pub max_response_bytes: usize,
}

impl Default for BoatHttpSettings {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(30),
            request_timeout: Duration::from_secs(60),
            max_response_bytes: 1024 * 1024,
        }
    }
}

impl BoatHttpSettings {
    pub(crate) fn validate(self) -> Result<()> {
        if self.connect_timeout.is_zero()
            || self.read_timeout.is_zero()
            || self.request_timeout.is_zero()
            || self.request_timeout > Duration::from_secs(60)
            || self.connect_timeout > self.request_timeout
            || self.read_timeout > self.request_timeout
            || self.max_response_bytes == 0
            || self.max_response_bytes > 1024 * 1024
        {
            return Err(Error::InvalidRequest);
        }
        Ok(())
    }
}
