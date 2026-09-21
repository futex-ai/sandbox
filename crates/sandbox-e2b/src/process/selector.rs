//! Atomic envd process selectors.

/// Provider-supported identity used by one process operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessSelector {
    /// Selects the process currently holding one numeric process ID.
    Pid(u32),
    /// Selects the process carrying one exact provider tag.
    Tag(String),
}
