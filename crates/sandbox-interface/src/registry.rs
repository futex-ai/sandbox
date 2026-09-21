//! Stable backend routing boundary.

use std::sync::Arc;

use crate::{DynSandboxBackend, Result};

/// Resolves configured backends by stable deployment ID.
#[unimock::unimock(api = SandboxBackendRegistryMock)]
pub trait SandboxBackendRegistry: Send + Sync {
    /// Returns the backend used for new profile-created sandboxes.
    fn default_backend_id(&self) -> &str;
    /// Lists every stable backend ID for bounded maintenance scans.
    fn backend_ids(&self) -> Vec<String>;
    /// Resolves a backend used by a new or stored resource.
    fn backend(&self, backend_id: &str) -> Result<DynSandboxBackend>;
    /// Returns true when a logical profile exists for the backend.
    fn has_profile(&self, backend_id: &str, profile: &str) -> bool;
    /// Returns true when a logical profile declares screen support.
    fn supports_screen(&self, backend_id: &str, profile: &str) -> bool;
    /// Returns the logical default profile for the backend.
    fn default_profile(&self, backend_id: &str) -> Result<String>;
}

/// Shared dynamic backend registry alias.
pub type DynSandboxBackendRegistry = Arc<dyn SandboxBackendRegistry>;
