//! Shared normal-completion predicate for provider helper processes.

use super::types::ProcessRunOutput;

impl ProcessRunOutput {
    pub(crate) fn succeeded(&self) -> bool {
        self.exited && self.exit_code == Some(0)
    }
}
