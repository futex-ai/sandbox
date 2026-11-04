//! Stable provider routing with independent capability access.

use std::sync::Arc;

use crate::{
    Result, SandboxAccountCapacity, SandboxFiles, SandboxImagePreparation, SandboxLifecycle,
    SandboxPortIngress, SandboxProcesses, SandboxResumableLifecycle, SandboxScreens,
    SandboxSnapshots, SandboxTerminals,
};

/// Resolves only the capabilities supported by each stable provider deployment.
#[unimock::unimock(api = SandboxRegistryMock)]
pub trait SandboxRegistry: Send + Sync {
    /// Returns the provider used for new profile-created sandboxes.
    fn default_backend_id(&self) -> &str;
    /// Lists every stable provider ID for bounded maintenance scans.
    fn backend_ids(&self) -> Vec<String>;
    /// Resolves the core lifecycle used by a new or stored sandbox.
    fn lifecycle(&self, backend_id: &str) -> Result<Arc<dyn SandboxLifecycle>>;
    /// Resolves pause and resume support, absent when the provider lacks it.
    fn resumable_lifecycle(
        &self,
        backend_id: &str,
    ) -> Result<Option<Arc<dyn SandboxResumableLifecycle>>>;
    /// Resolves saved snapshot support, absent when the provider lacks it.
    fn snapshots(&self, backend_id: &str) -> Result<Option<Arc<dyn SandboxSnapshots>>>;
    /// Resolves image preparation support, absent when the provider lacks it.
    fn image_preparation(
        &self,
        backend_id: &str,
    ) -> Result<Option<Arc<dyn SandboxImagePreparation>>>;
    /// Resolves process execution support, absent when the provider lacks it.
    fn processes(&self, backend_id: &str) -> Result<Option<Arc<dyn SandboxProcesses>>>;
    /// Resolves regular-file transfer support, absent when the provider lacks it.
    fn files(&self, backend_id: &str) -> Result<Option<Arc<dyn SandboxFiles>>>;
    /// Resolves terminal support, absent when the provider lacks it.
    fn terminals(&self, backend_id: &str) -> Result<Option<Arc<dyn SandboxTerminals>>>;
    /// Resolves private port support, absent when the provider lacks it.
    fn port_ingress(&self, backend_id: &str) -> Result<Option<Arc<dyn SandboxPortIngress>>>;
    /// Resolves screen support, absent when the provider lacks it.
    fn screens(&self, backend_id: &str) -> Result<Option<Arc<dyn SandboxScreens>>>;
    /// Resolves advisory account capacity, absent when the provider lacks it.
    fn account_capacity(&self, backend_id: &str)
    -> Result<Option<Arc<dyn SandboxAccountCapacity>>>;
    /// Returns true when a logical profile exists for the provider.
    fn has_profile(&self, backend_id: &str, profile: &str) -> bool;
    /// Returns true when a logical profile declares screen support.
    fn supports_screen(&self, backend_id: &str, profile: &str) -> bool;
    /// Returns the logical default profile for the provider.
    fn default_profile(&self, backend_id: &str) -> Result<String>;
}
