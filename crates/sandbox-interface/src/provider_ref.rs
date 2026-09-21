//! Opaque sandbox-provider identity.

use serde::{Deserialize, Serialize};

/// Opaque provider identifier retained only below the service boundary.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ProviderRef(String);

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
