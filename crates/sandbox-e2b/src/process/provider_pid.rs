//! Validated operating-system process identities decoded from provider responses.

use std::num::NonZeroU32;

use serde::Deserialize;

/// A provider process ID that is safe to expose to process operations.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(transparent)]
pub(super) struct ProviderPid(NonZeroU32);

impl ProviderPid {
    /// Returns the validated process ID as the public transport representation.
    pub(super) const fn get(self) -> u32 {
        self.0.get()
    }
}
