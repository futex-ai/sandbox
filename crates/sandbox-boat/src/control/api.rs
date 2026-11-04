//! Injectable object-safe Boat control capability.

use async_trait::async_trait;

use crate::error::Result;

use super::types::{
    BoatCreateSandbox, BoatLabels, BoatLimits, BoatSandbox, BoatSaveSnapshot, BoatSavedSnapshot,
};

/// Provider control operations. Implementations never replay an uncertain create.
#[unimock::unimock(api = BoatControlApiMock)]
#[async_trait]
pub trait BoatControlApi: Send + Sync {
    /// Dispatches one sandbox start without provider preflight.
    async fn create_sandbox(&self, request: BoatCreateSandbox) -> Result<BoatSandbox>;
    /// Reads bounded, paginated inventory with the supplied labels.
    async fn list_sandboxes(&self, labels: BoatLabels) -> Result<Vec<BoatSandbox>>;
    /// Reads one sandbox without extending its TTL.
    async fn get_sandbox(&self, id: &str) -> Result<BoatSandbox>;
    /// Repeats sandbox DELETE until completion or terminal absence.
    async fn delete_sandbox(&self, id: &str) -> Result<()>;
    /// Reads one exact saved-snapshot name.
    async fn get_snapshot(&self, name: &str) -> Result<BoatSavedSnapshot>;
    /// Dispatches one saved-snapshot capture.
    async fn save_snapshot(&self, request: BoatSaveSnapshot) -> Result<BoatSavedSnapshot>;
    /// Removes a saved snapshot with its dedicated acknowledgment.
    async fn delete_snapshot(&self, name: &str) -> Result<()>;
    /// Reads account-scoped advisory eligibility.
    async fn limits(&self) -> Result<BoatLimits>;
}
