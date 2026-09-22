//! Outbound allowlist conformance-command regressions.

use super::{ALLOWED_FETCH, DISALLOWED_FETCH};

#[test]
fn disallowed_fetch_does_not_turn_http_errors_into_denial_success() {
    assert!(
        ALLOWED_FETCH
            .split_ascii_whitespace()
            .any(|arg| arg == "--fail")
    );
    assert!(
        !DISALLOWED_FETCH
            .split_ascii_whitespace()
            .any(|arg| arg == "--fail")
    );
}
