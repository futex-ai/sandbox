//! Opaque sandbox-provider identity.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Opaque provider identifier retained only below the service boundary.
#[derive(Clone, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ProviderRef(String);

impl fmt::Debug for ProviderRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProviderRef([redacted])")
    }
}

impl ProviderRef {
    /// Wraps a non-model-visible provider identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrows the opaque identifier for provider dispatch.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
