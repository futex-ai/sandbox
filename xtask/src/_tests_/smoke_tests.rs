//! Credential-free smoke construction coverage.

use super::construct;

#[test]
fn accepts_placeholder_credentials_without_provider_dispatch() {
    construct("https://api.e2b.app", "placeholder-api-key").unwrap();
}

#[test]
fn rejects_invalid_configuration_before_backend_construction() {
    assert!(construct("http://api.e2b.app", "placeholder-api-key").is_err());
}
