//! E2B-specific one-shot lifetime bound shared by backend and public control creation.

use std::time::Duration;

use sandbox_interface::{Error, Result, SandboxLifetime};

/// Maximum destruction timeout supported by E2B one-shot sandboxes.
pub const E2B_ONE_SHOT_MAX_LIFETIME: Duration = Duration::from_secs(3600);

/// Keeps E2B's limit independent of the shared provider request representation.
pub(crate) fn validate(lifetime: SandboxLifetime) -> Result<()> {
    if let SandboxLifetime::OneShot { max_lifetime } = lifetime
        && (max_lifetime.is_zero()
            || max_lifetime > E2B_ONE_SHOT_MAX_LIFETIME
            || max_lifetime.subsec_nanos() != 0)
    {
        return Err(Error::InvalidSeconds {
            field: "max_lifetime",
            minimum: 1,
            maximum: E2B_ONE_SHOT_MAX_LIFETIME.as_secs(),
        });
    }
    lifetime.validate()
}
