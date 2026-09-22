//! Sandbox lifetime validation and metadata regressions.

use std::time::Duration;

use crate::{Error, SANDBOX_ONE_SHOT_MAX_LIFETIME, SandboxLifetime};

#[test]
fn idle_auto_pause_is_the_default_lifetime() {
    assert_eq!(SandboxLifetime::default(), SandboxLifetime::IdleAutoPause);
}

#[test]
fn one_shot_lifetime_accepts_the_inclusive_shared_bounds() {
    for max_lifetime in [Duration::from_secs(1), SANDBOX_ONE_SHOT_MAX_LIFETIME] {
        let lifetime = SandboxLifetime::OneShot { max_lifetime };

        lifetime.validate().expect("inclusive lifetime bound");
        assert_eq!(
            lifetime
                .one_shot_timeout_seconds()
                .expect("validated seconds"),
            Some(max_lifetime.as_secs() as u32)
        );
    }
}

#[test]
fn one_shot_lifetime_rejects_zero_over_limit_and_fractional_seconds() {
    for max_lifetime in [
        Duration::ZERO,
        SANDBOX_ONE_SHOT_MAX_LIFETIME + Duration::from_secs(1),
        Duration::from_millis(1500),
    ] {
        assert!(matches!(
            SandboxLifetime::OneShot { max_lifetime }.validate(),
            Err(Error::InvalidSeconds {
                field: "max_lifetime",
                minimum: 1,
                maximum: 3600,
            })
        ));
    }
}

#[test]
fn lifetime_metadata_round_trips_without_guessing_incomplete_values() {
    let one_shot = SandboxLifetime::OneShot {
        max_lifetime: Duration::from_secs(90),
    };

    assert_eq!(
        SandboxLifetime::from_metadata(one_shot.metadata_kind(), Some("90")),
        Some(one_shot)
    );
    assert_eq!(
        SandboxLifetime::from_metadata(SandboxLifetime::IdleAutoPause.metadata_kind(), None),
        Some(SandboxLifetime::IdleAutoPause)
    );
    assert_eq!(SandboxLifetime::from_metadata("one_shot", None), None);
    assert_eq!(
        SandboxLifetime::from_metadata("idle_auto_pause", Some("90")),
        None
    );
    assert_eq!(
        SandboxLifetime::from_metadata("one_shot", Some("090")),
        None
    );
    assert_eq!(
        SandboxLifetime::from_metadata("one_shot", Some("3601")),
        None
    );
    assert_eq!(SandboxLifetime::from_metadata("unknown", Some("90")), None);
}
