//! The sun's brightness curve.
//!
//! Source: `sun_light_at` (`src/calendar.cpp:381-402`). The expected values below were
//! computed from that expression rather than estimated, and the tolerance covers float
//! rounding on both sides.

// Exact comparisons where the reference is exact: night is zero and the top of the curve is
// a constant, so a tolerance there would only hide a branch that returned something else.
#![allow(clippy::panic, clippy::float_cmp)]

use cdda_core_types::light::{FULL_DAYLIGHT, sun_light_from_altitude};

/// Altitude in degrees, and the brightness the reference's curve gives there.
const CURVE: [(f32, f32); 12] = [
    (-20.0, 0.0),
    (-18.0, 0.0),
    (-15.0, 1.532_59),
    (-12.0, 3.7),
    (-9.0, 4.238_478),
    (-6.0, 5.0),
    (-3.0, 33.364_41),
    (-1.0, 60.0),
    (0.0, 61.065_57),
    (30.0, 93.032_79),
    (60.0, 125.0),
    (90.0, 125.0),
];

#[test]
fn the_sun_follows_the_reference_curve() {
    for (altitude, expected) in CURVE {
        let actual = sun_light_from_altitude(altitude);
        assert!(
            (actual - expected).abs() < 1e-3,
            "altitude {altitude}: expected {expected}, got {actual}"
        );
    }
}

#[test]
fn the_curve_rises_from_nothing_to_full_daylight_without_going_backwards() {
    // Monotonicity is not obvious from four branches with different shapes, and a wrong
    // branch boundary shows up as a dip at dawn - the hour when a dip is most visible.
    let mut previous = -1.0;
    let mut altitude = -25.0;
    while altitude <= 90.0 {
        let light = sun_light_from_altitude(altitude);
        assert!(
            light >= previous - 1e-4,
            "brightness fell at {altitude}: {previous} then {light}"
        );
        assert!(
            (0.0..=FULL_DAYLIGHT).contains(&light),
            "{light} out of range"
        );
        previous = light;
        altitude += 0.5;
    }
    assert_eq!(
        previous, FULL_DAYLIGHT,
        "the top of the curve is full daylight"
    );
}

#[test]
fn night_is_black_and_noon_is_flat() {
    assert_eq!(sun_light_from_altitude(-40.0), 0.0, "deep night");
    assert_eq!(
        sun_light_from_altitude(60.0),
        FULL_DAYLIGHT,
        "the sun reaches its peak"
    );
    assert_eq!(
        sun_light_from_altitude(89.0),
        sun_light_from_altitude(60.0),
        "above sixty degrees nothing changes"
    );
}

#[test]
fn the_sky_becomes_a_light_level_at_the_thresholds_the_rest_of_the_game_uses() {
    use cdda_core_types::light::{LitLevel, open_sky_lit_level};

    // Full daylight is far past the lit threshold.
    assert_eq!(open_sky_lit_level(30.0), LitLevel::Lit);
    // A bright dawn is still lit.
    assert_eq!(open_sky_lit_level(-3.0), LitLevel::Lit);
    // Just before nautical dawn the curve gives 1.53, which is under AMBIENT_LOW: dark.
    assert_eq!(open_sky_lit_level(-15.0), LitLevel::Dark);
    // Deep night is dark, and there is a band between that is merely hard to see by.
    assert_eq!(open_sky_lit_level(-40.0), LitLevel::Dark);
    assert_eq!(
        open_sky_lit_level(-9.0),
        LitLevel::Low,
        "4.24 is above AMBIENT_LOW and below full daylight"
    );
}
