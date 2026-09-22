//! Secret-safe port-ingress value coverage.

use super::{PortIngress, PortIngressCredential};

#[test]
fn ingress_debug_output_redacts_provider_url_and_credential() {
    let ingress = PortIngress::new(
        "https://4173-provider-secret.example",
        Some(PortIngressCredential::new(
            "provider-auth",
            "traffic-token-secret",
        )),
    );

    let debug = format!("{ingress:?}");

    assert!(debug.contains("provider-auth"));
    assert!(!debug.contains("provider-secret"));
    assert!(!debug.contains("traffic-token-secret"));
}

#[test]
fn ingress_values_are_available_only_through_explicit_accessors() {
    let ingress = PortIngress::new(
        "https://4173-provider.example",
        Some(PortIngressCredential::new("provider-auth", "traffic-token")),
    );

    assert_eq!(ingress.upstream_url(), "https://4173-provider.example");
    let credential = ingress.credential().expect("credential");
    assert_eq!(credential.header_name(), "provider-auth");
    assert_eq!(credential.header_value(), "traffic-token");
}
