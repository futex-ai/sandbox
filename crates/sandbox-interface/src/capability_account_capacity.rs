//! Advisory sizes and prices for the provider's configured billing account.

use async_trait::async_trait;

use crate::{Result, SandboxSizeOffer};

/// Reports account-eligible sizes without reserving or starting a sandbox.
#[unimock::unimock(api = SandboxAccountCapacityMock)]
#[async_trait]
pub trait SandboxAccountCapacity: Send + Sync {
    /// Returns only eligible sizes with known resource amounts and hourly prices.
    ///
    /// Uses the same configured account or organization billed by create.
    /// Eligibility is advisory. Only the create result determines whether a
    /// later start succeeded. An empty list means no eligible known sizes.
    async fn account_capacity(&self) -> Result<Vec<SandboxSizeOffer>>;
}
