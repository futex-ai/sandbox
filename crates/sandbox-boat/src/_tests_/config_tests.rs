//! Canonical account scope names remain independent of Bearer token rules.

use std::collections::BTreeMap;

use crate::{
    config::{BoatAdapterConfig, BoatProfile},
    error::Error,
};

#[test]
fn organization_names_keep_internal_spaces_while_keys_stay_canonical() {
    let profiles = BTreeMap::from([(
        "general".to_owned(),
        BoatProfile {
            provider_size: "default".to_owned(),
            allow_public_egress: true,
            denied_destinations: Vec::new(),
        },
    )]);
    let config =
        BoatAdapterConfig::new("boat", "https://boat.dev", "boat_key", profiles.clone()).unwrap();
    let organization = config.with_organization("Example Research Team");
    assert!(
        matches!(&organization, Ok(config) if config.organization() == Some("Example Research Team")),
        "documented organization names must preserve their internal spaces"
    );
    let padded_key =
        BoatAdapterConfig::new("boat", "https://boat.dev", "key with spaces", profiles);
    assert!(matches!(padded_key, Err(Error::InvalidRequest)));
}
