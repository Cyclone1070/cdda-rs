//! How light fades as it travels, and how a swept span's transparency is averaged.
//!
//! Reference: `sight_calc`, `sight_check`, `accumulate_transparency` and `update_light` (`src/shadowcasting.h:96-116`). These four tiny functions are the whole
//! physics and the whole bookkeeping of the light cast; everything else in `shadowcasting.cpp` is deciding which squares a ray passes through.
//!
//! The attenuation rule is the Beer-Lambert law with the length factored out - `1 / e^( a*l )` - and the absorption coefficient `a` is the **running mean** of the
//! transparencies the ray has passed through, which is what makes the cast a sweep of spans rather than a per-ray walk.

#![allow(clippy::expect_used)]

use cdda_core_types::light::{AMBIENT_LIT, SOURCE_BRIGHT, TRANSPARENCY_OPEN_AIR};
use cdda_core_types::shadowcasting::{
    accumulate_transparency, sight_calc, sight_check, update_light,
};

/// Floats: "the same" means "as close as floats get".
#[track_caller]
fn assert_close(passed: f32, expected: f32, what: &str) {
    assert!(
        (passed - expected).abs() < 1e-5,
        "{what}: got {passed}, expected {expected}"
    );
}

#[test]
fn open_air_eats_almost_everything_in_seventy_two_squares() {
    // `TRANSPARENCY_OPEN_AIR` is `2.3 / 72`, chosen so that `e^-2.3` - about a tenth - is what is left at `MAX_VIEW_DISTANCE`. The values below were computed from
    // `1 / exp( 2.3 * d / 72 )` independently of this code.
    assert_close(
        sight_calc(1.0, TRANSPARENCY_OPEN_AIR, 1),
        0.968_560_4,
        "one square",
    );
    assert_close(
        sight_calc(1.0, TRANSPARENCY_OPEN_AIR, 10),
        0.726_552_6,
        "ten squares",
    );
    assert_close(
        sight_calc(1.0, TRANSPARENCY_OPEN_AIR, 72),
        0.100_258_8,
        "seventy-two squares: a tenth",
    );
    assert_close(
        sight_calc(SOURCE_BRIGHT, TRANSPARENCY_OPEN_AIR, 72),
        1.002_588_4,
        "a bright lamp is exactly at the lit threshold at the edge of sight",
    );
}

#[test]
fn solid_terrain_takes_nothing_away_and_takes_everything_away() {
    // A transparency of zero means no attenuation at all - `1 / e^0` - which is why the *blocking* is done by `sight_check` refusing to walk into the square rather
    // than by the arithmetic dropping to nothing. A port that made `sight_calc` return zero for a wall would be wrong by one square of light.
    assert_close(
        sight_calc(AMBIENT_LIT, 0.0, 40),
        AMBIENT_LIT,
        "no absorption",
    );
    assert!(!sight_check(0.0), "but a wall is not walked into");
    assert!(sight_check(f32::MIN_POSITIVE), "and anything above zero is");
}

#[test]
fn the_running_average_is_the_mean_of_everything_the_ray_has_seen() {
    // `accumulate_transparency` (`shadowcasting.h:112-116`): one step at each distance, ending at the exact arithmetic mean of the samples so far. It follows that the
    // answer does not depend on the order, which is a property worth pinning: a sweep that met the same squares in another order would still get the same light.
    let mut forwards = 2.3 / 72.0;
    let mut backwards = 2.3 / 72.0;
    for (distance, (forward, backward)) in [(0.5_f32, 2.0_f32), (1.0, 1.0), (2.0, 0.5)]
        .into_iter()
        .zip([2, 3, 4])
        .map(|(pair, distance)| (distance, pair))
    {
        forwards = accumulate_transparency(forwards, forward, distance);
        backwards = accumulate_transparency(backwards, backward, distance);
    }
    assert_close(forwards, backwards, "the order of the four does not matter");
    let mean = (2.3 / 72.0 + 0.5 + 1.0 + 2.0) / 4.0;
    assert_close(forwards, mean, "the mean of the four");
}

#[test]
fn the_first_step_of_a_ray_is_the_transparency_it_meets() {
    // `( ( 1 - 1 ) * cumulative + current ) / 1`. A port that carried the previous value into the first step would light the whole map from the origin's own square.
    assert_close(
        accumulate_transparency(999.0, 0.25, 1),
        0.25,
        "the cumulative value is ignored at the first step",
    );
}

#[test]
fn a_square_keeps_the_light_it_already_had() {
    // `update_light` (`shadowcasting.h:105-108`) is a maximum, and it is written with an explicit `if` rather than `std::max` for a compiler reason the reference
    // explains. Same answer either way.
    assert_close(update_light(3.0, 1.0), 3.0, "dimmer light changes nothing");
    assert_close(update_light(3.0, 5.0), 5.0, "brighter light wins");
    assert_close(update_light(3.0, 3.0), 3.0, "equal is equal");
}
