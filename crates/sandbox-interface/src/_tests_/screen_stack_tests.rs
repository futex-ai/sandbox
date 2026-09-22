//! Screen viewport value-contract coverage.

use crate::{
    Error, SCREEN_VIEWPORT_MAX_HEIGHT, SCREEN_VIEWPORT_MAX_PIXELS, SCREEN_VIEWPORT_MAX_WIDTH,
    SCREEN_VIEWPORT_MIN_HEIGHT, SCREEN_VIEWPORT_MIN_WIDTH, ScreenViewportSize,
};

#[test]
fn supported_viewport_pairs_are_preserved_exactly() {
    let representatives = [
        (320, 240),
        (390, 700),
        (1280, 800),
        (2560, 1080),
        (3840, 2160),
        (3840, 2159),
        (3839, 2160),
    ];

    for (width, height) in representatives {
        let viewport = ScreenViewportSize::new(width, height).expect("supported viewport");
        assert_eq!(viewport.width(), width);
        assert_eq!(viewport.height(), height);
    }
}

#[test]
fn every_boundary_adjacent_invalid_partition_is_rejected() {
    let invalid = [
        (SCREEN_VIEWPORT_MIN_WIDTH - 1, 800),
        (SCREEN_VIEWPORT_MAX_WIDTH + 1, 800),
        (1280, SCREEN_VIEWPORT_MIN_HEIGHT - 1),
        (1280, SCREEN_VIEWPORT_MAX_HEIGHT + 1),
        (SCREEN_VIEWPORT_MAX_WIDTH, SCREEN_VIEWPORT_MAX_HEIGHT + 1),
    ];

    for (width, height) in invalid {
        assert!(matches!(
            ScreenViewportSize::new(width, height),
            Err(Error::InvalidScreenViewport { .. })
        ));
    }
}

#[test]
fn pixel_product_limit_is_enforced_without_axis_clamping() {
    let at_limit = ScreenViewportSize::new(3840, 2160).expect("4K is supported");
    assert_eq!(
        u64::from(at_limit.width()) * u64::from(at_limit.height()),
        SCREEN_VIEWPORT_MAX_PIXELS
    );

    assert!(matches!(
        ScreenViewportSize::new(3840, 2161),
        Err(Error::InvalidScreenViewport {
            width: 3840,
            height: 2161
        })
    ));
}

#[test]
fn viewport_deserialization_cannot_bypass_validation() {
    let viewport = serde_json::from_str::<ScreenViewportSize>(r#"{"width":319,"height":800}"#);

    assert!(viewport.is_err());
}
