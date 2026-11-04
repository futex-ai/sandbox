//! Typed sandbox lifetime policies and shared whole-second representability.

use std::time::Duration;

use crate::{Error, Result};

/// Provider-neutral lifetime policy selected when a sandbox is created.
///
/// A one-shot sandbox never pauses and is not resumable. Its consumer is
/// expected to destroy it after the bounded work completes; the provider
/// destroys it when `max_lifetime` expires if the consumer does not.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SandboxLifetime {
    /// Pause after the configured idle timeout and allow an explicit resume.
    #[default]
    IdleAutoPause,
    /// Stay running until explicitly destroyed or the provider timeout expires.
    OneShot {
        /// Positive whole-second timeout representable as `u32` seconds.
        max_lifetime: Duration,
    },
}

impl SandboxLifetime {
    /// Validates positive whole seconds representable by provider request DTOs.
    ///
    /// Each adapter must also validate its own maximum before provider access.
    pub fn validate(self) -> Result<()> {
        let Self::OneShot { max_lifetime } = self else {
            return Ok(());
        };
        if max_lifetime.is_zero()
            || max_lifetime.as_secs() > u64::from(u32::MAX)
            || max_lifetime.subsec_nanos() != 0
        {
            return Err(Error::InvalidSeconds {
                field: "max_lifetime",
                minimum: 1,
                maximum: u64::from(u32::MAX),
            });
        }
        Ok(())
    }

    /// Returns the stable lifetime-kind value stored in provider metadata.
    #[must_use]
    pub const fn metadata_kind(self) -> &'static str {
        match self {
            Self::IdleAutoPause => "idle_auto_pause",
            Self::OneShot { .. } => "one_shot",
        }
    }

    /// Returns the validated one-shot timeout in provider whole seconds.
    pub fn one_shot_timeout_seconds(self) -> Result<Option<u32>> {
        self.validate()?;
        match self {
            Self::IdleAutoPause => Ok(None),
            Self::OneShot { max_lifetime } => Ok(Some(max_lifetime.as_secs() as u32)),
        }
    }

    /// Parses a complete provider-metadata representation.
    ///
    /// Missing, unknown, incomplete, or out-of-range values remain unknown.
    #[must_use]
    pub fn from_metadata(kind: &str, one_shot_seconds: Option<&str>) -> Option<Self> {
        let lifetime = match kind {
            "idle_auto_pause" if one_shot_seconds.is_none() => Self::IdleAutoPause,
            "one_shot" => {
                let encoded_seconds = one_shot_seconds?;
                let seconds = encoded_seconds.parse::<u64>().ok()?;
                if seconds.to_string() != encoded_seconds {
                    return None;
                }
                Self::OneShot {
                    max_lifetime: Duration::from_secs(seconds),
                }
            }
            _ => return None,
        };
        lifetime.validate().ok().map(|()| lifetime)
    }
}

#[cfg(test)]
#[path = "_tests_/lifetime_tests.rs"]
mod lifetime_tests;
