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

#[test]
fn fetches_disable_curl_startup_configuration() {
    for command in [ALLOWED_FETCH, DISALLOWED_FETCH] {
        assert_eq!(
            command.split_ascii_whitespace().take(2).collect::<Vec<_>>(),
            ["curl", "--disable"]
        );
    }
}
