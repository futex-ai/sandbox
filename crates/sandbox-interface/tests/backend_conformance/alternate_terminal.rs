//! Shared terminal identity for the alternate conformance backend.

use sandbox_interface::{BackendTerminal, ProviderRef, TerminalState};

pub(super) fn terminal(provider_ref: ProviderRef) -> BackendTerminal {
    BackendTerminal {
        provider_ref,
        provider_log_path: "/tmp/alternate.log".to_owned(),
        state: TerminalState::Ready,
    }
}
