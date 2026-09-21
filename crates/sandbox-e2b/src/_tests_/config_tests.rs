//! Deployment-owned E2B network-policy validation tests.

use std::collections::HashMap;

use sandbox_interface::{SANDBOX_PROFILE_MAX_BYTES, SANDBOX_PROFILE_MAX_ITEMS};

use crate::{
    E2bAdapterConfig, E2bAdapterError, E2bAdapterResult, E2bProfile, E2bRuntimeConventions,
};

#[test]
fn runtime_conventions_have_neutral_defaults() {
    let conventions = E2bRuntimeConventions::default();

    assert_eq!(conventions.metadata_prefix(), "sandbox");
    assert_eq!(conventions.terminal_tag_prefix(), "sandbox-terminal-");
    assert_eq!(
        conventions.screen_helper_path(),
        "/usr/local/bin/sandbox-screen"
    );
    assert_eq!(conventions.image_helper_process_name(), "sandbox-helper");
    assert_eq!(conventions.image_agent_process_name(), "sandbox-agent");
}

#[test]
fn validated_configuration_exposes_read_only_values() {
    let config = config(vec!["203.0.113.10/32".to_owned()]).expect("valid config");

    assert_eq!(config.backend_id(), "e2b");
    assert_eq!(config.api_base(), "https://api.e2b.app");
    assert_eq!(config.idle_timeout_seconds(), 600);
    assert_eq!(config.profiles().len(), 1);
    assert_eq!(
        config.runtime_conventions(),
        &E2bRuntimeConventions::default()
    );
}

#[test]
fn runtime_conventions_accept_deployment_owned_values() {
    let conventions = E2bRuntimeConventions::new(
        "tenant_7",
        "tenant-terminal-",
        "/opt/tenant/bin/screen-helper",
    )
    .unwrap()
    .with_image_process_names("tenant-helper", "tenant-agent")
    .unwrap();
    let config = config(vec!["203.0.113.10/32".to_owned()])
        .unwrap()
        .with_runtime_conventions(conventions.clone());

    assert_eq!(config.runtime_conventions(), &conventions);
    assert_eq!(
        config.runtime_conventions().image_helper_process_name(),
        "tenant-helper"
    );
    assert_eq!(
        config.runtime_conventions().image_agent_process_name(),
        "tenant-agent"
    );
}

#[test]
fn runtime_conventions_reject_unsafe_image_process_names() {
    for (helper, agent) in [
        ("", "sandbox-agent"),
        ("helper name", "sandbox-agent"),
        ("../helper", "sandbox-agent"),
        ("sandbox-helper", "agent|other"),
        ("sandbox-helper", "a-name-that-is-too-long"),
    ] {
        assert!(matches!(
            E2bRuntimeConventions::default().with_image_process_names(helper, agent),
            Err(E2bAdapterError::InvalidRequest)
        ));
    }
}

#[test]
fn runtime_conventions_reject_unsafe_or_ambiguous_values() {
    for (metadata, terminal, helper) in [
        ("Tenant", "tenant-terminal-", "/opt/screen-helper"),
        ("tenant_", "tenant-terminal-", "/opt/screen-helper"),
        ("tenant__blue", "tenant-terminal-", "/opt/screen-helper"),
        ("tenant", "terminal", "/opt/screen-helper"),
        ("tenant", "tenant-terminal-", "relative/screen-helper"),
        ("tenant", "tenant-terminal-", "/opt/../screen-helper"),
        ("tenant", "tenant-terminal-", "/opt//screen-helper"),
        ("tenant", "tenant-terminal-", "/"),
        ("tenant", "tenant-terminal-", "/opt/screen\0helper"),
    ] {
        assert!(matches!(
            E2bRuntimeConventions::new(metadata, terminal, helper),
            Err(E2bAdapterError::InvalidRequest)
        ));
    }
}

#[test]
fn adapter_rejects_missing_or_unsupported_deny_destinations() {
    assert!(matches!(
        config(Vec::new()),
        Err(E2bAdapterError::InvalidRequest)
    ));
    assert!(matches!(
        config(vec!["not-an-ip".to_owned()]),
        Err(E2bAdapterError::InvalidRequest)
    ));
    assert!(matches!(
        config(vec!["203.0.113.10/33".to_owned()]),
        Err(E2bAdapterError::InvalidRequest)
    ));
}

#[test]
fn adapter_accepts_bare_ips_and_cidrs_for_both_address_families() {
    config(vec![
        "203.0.113.10".to_owned(),
        "203.0.113.0/24".to_owned(),
        "2001:db8::10".to_owned(),
        "2001:db8::/64".to_owned(),
    ])
    .expect("E2B-supported deny destinations should validate");
}

#[test]
fn adapter_canonicalizes_whitespace_and_duplicate_destinations() {
    let config = config(vec![
        " 203.0.113.0/24 ".to_owned(),
        "203.0.113.0/024".to_owned(),
        " 2001:0db8::10 ".to_owned(),
    ])
    .expect("padded IP destinations should be canonicalized");

    assert_eq!(
        config
            .profile("general")
            .expect("general profile")
            .denied_destinations,
        vec!["2001:db8::10".to_owned(), "203.0.113.0/24".to_owned()]
    );
}

#[test]
fn adapter_rejects_invalid_api_bases() {
    for api_base in [
        "not-a-url",
        "ftp://api.e2b.app",
        "http://api.e2b.app",
        "https://",
        "https://api.e2b.app/proxy",
    ] {
        assert!(matches!(
            E2bAdapterConfig::new(
                "e2b",
                api_base,
                "api-key",
                profiles(vec!["203.0.113.10/32".to_owned()]),
                600,
            ),
            Err(E2bAdapterError::InvalidRequest)
        ));
    }
}

#[test]
fn adapter_rejects_non_canonical_profile_keys() {
    for profile_name in ["", " ", " general", "general ", "Browser", "python_3"] {
        let profiles = HashMap::from([(
            profile_name.to_owned(),
            E2bProfile {
                template: "base".to_owned(),
                allow_public_egress: true,
                denied_destinations: vec!["203.0.113.10/32".to_owned()],
            },
        )]);

        assert!(matches!(
            E2bAdapterConfig::new("e2b", "https://api.e2b.app", "api-key", profiles, 600,),
            Err(E2bAdapterError::InvalidRequest)
        ));
    }
}

#[test]
fn adapter_enforces_the_shared_profile_catalog_bound() {
    let profiles = (0..=SANDBOX_PROFILE_MAX_ITEMS)
        .map(|index| {
            (
                format!("env-{index}"),
                E2bProfile {
                    template: format!("template-{index}"),
                    allow_public_egress: true,
                    denied_destinations: vec!["203.0.113.10/32".to_owned()],
                },
            )
        })
        .collect();

    assert!(matches!(
        E2bAdapterConfig::new("e2b", "https://api.e2b.app", "api-key", profiles, 600),
        Err(E2bAdapterError::InvalidRequest)
    ));
}

#[test]
fn adapter_rejects_padded_identity_and_provider_values() {
    for backend_id in [" e2b", "e2b "] {
        assert_invalid_adapter_values(backend_id, "api-key", "base");
    }
    for api_key in [" api-key", "api-key "] {
        assert_invalid_adapter_values("e2b", api_key, "base");
    }
    for template in [" base", "base "] {
        assert_invalid_adapter_values("e2b", "api-key", template);
    }
}

#[test]
fn adapter_rejects_profile_keys_over_the_runtime_byte_limit() {
    let profile_name = "x".repeat(SANDBOX_PROFILE_MAX_BYTES + 1);
    let profiles = HashMap::from([(
        profile_name,
        E2bProfile {
            template: "base".to_owned(),
            allow_public_egress: true,
            denied_destinations: vec!["203.0.113.10/32".to_owned()],
        },
    )]);

    assert!(matches!(
        E2bAdapterConfig::new("e2b", "https://api.e2b.app", "api-key", profiles, 600,),
        Err(E2bAdapterError::InvalidRequest)
    ));
}

fn assert_invalid_adapter_values(backend_id: &str, api_key: &str, template: &str) {
    let profiles = HashMap::from([(
        "general".to_owned(),
        E2bProfile {
            template: template.to_owned(),
            allow_public_egress: true,
            denied_destinations: vec!["203.0.113.10/32".to_owned()],
        },
    )]);

    assert!(matches!(
        E2bAdapterConfig::new(backend_id, "https://api.e2b.app", api_key, profiles, 600,),
        Err(E2bAdapterError::InvalidRequest)
    ));
}

fn config(denied_destinations: Vec<String>) -> E2bAdapterResult<E2bAdapterConfig> {
    E2bAdapterConfig::new(
        "e2b",
        "https://api.e2b.app",
        "api-key",
        profiles(denied_destinations),
        600,
    )
}

fn profiles(denied_destinations: Vec<String>) -> HashMap<String, E2bProfile> {
    HashMap::from([(
        "general".to_owned(),
        E2bProfile {
            template: "base".to_owned(),
            allow_public_egress: true,
            denied_destinations,
        },
    )])
}
