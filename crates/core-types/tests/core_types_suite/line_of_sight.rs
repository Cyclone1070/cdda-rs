//! Whether the player can see one particular square.
//!
//! Reference: `map::pl_sees( t, max_range )` (`src/lightmap.cpp:859-877`), the function the rest of the game asks instead of re-deriving visibility. It composes
//! everything below it - the range check, the apparent light, the player's own threshold and the light source sitting in the square - and adds nothing of its own
//! except the order.
//!
//! Two distance metrics meet here and they are **not** the same one. The range check uses `square_dist` - Chebyshev, the larger of the two offsets - while
//! `apparent_light_at` uses `rl_dist`, which is Chebyshev only when the "circular distances" option is off and Euclidean when it is on. A square can therefore be inside
//! the range check and outside the light's own idea of distance, and the reference has always been that way.

#![allow(clippy::expect_used)]

use cdda_core_types::light::{Target, rl_dist, sees, square_dist};

#[track_caller]
fn assert_close(passed: f32, expected: f32, what: &str) {
    assert!(
        (passed - expected).abs() < 1e-5,
        "{what}: got {passed}, expected {expected}"
    );
}

fn visible() -> Target {
    Target {
        in_bounds: true,
        square_dist: 3,
        camera: 0.0,
        obstructed: false,
        light: 5.0,
        light_source: 0.0,
    }
}

#[test]
fn outside_the_map_nothing_is_seen_whatever_else_is_true() {
    // The bounds check is first and unconditional (`lightmap.cpp:861-863`). A square off the map has no cache to read, so this is not politeness - it is what makes
    // the rest of the function safe.
    let mut square = visible();
    square.in_bounds = false;
    assert!(!sees(60, &square, 3.5));
}

#[test]
fn the_range_check_is_the_larger_offset_and_is_strictly_greater() {
    // `square_dist( ... ) > max_range` (`lightmap.cpp:865-868`): at exactly the range the square is still seen, one past it is not - unless a camera is watching, which
    // is the only thing that bypasses the check.
    let mut at_range = visible();
    at_range.square_dist = 10;
    assert!(sees(10, &at_range, 3.5), "exactly at the range");

    at_range.square_dist = 11;
    assert!(!sees(10, &at_range, 3.5), "one square past it");

    at_range.camera = 0.5;
    assert!(
        sees(10, &at_range, 3.5),
        "a camera at the square puts it back in range"
    );
}

#[test]
fn a_negative_range_means_no_range_limit_at_all() {
    // Only `max_range >= 0` turns the check on, so the caller that does not care about distance passes a negative number.
    let mut far = visible();
    far.square_dist = 500;
    assert!(sees(-1, &far, 3.5));
}

#[test]
fn an_obstructed_square_is_not_seen_even_when_it_is_lit() {
    // `!a.obstructed && ...`: obstruction comes first and is not weighed against the light. This is the one place where the light being enough does **not** help,
    // which is why `apparent_light_at` has a separate `BRIGHT_ONLY` answer for light that gets through without detail.
    let mut blocked = visible();
    blocked.light = 100.0;
    blocked.obstructed = true;
    assert!(!sees(60, &blocked, 3.5));
}

#[test]
fn enough_light_or_a_light_source_in_the_square_either_will_do() {
    let mut dim = visible();
    dim.light = 2.0;
    assert!(!sees(60, &dim, 3.5), "below the threshold and no lamp");

    dim.light = 3.5;
    assert!(sees(60, &dim, 3.5), "at the threshold exactly");

    dim.light = 0.0;
    dim.light_source = 0.1;
    assert!(
        sees(60, &dim, 3.5),
        "a lamp in the square is visible even with no light around it"
    );
}

#[test]
fn the_two_distance_metrics_are_not_the_same_metric() {
    // `square_dist` is Chebyshev, always (`line.h:182-191`). `rl_dist` is Chebyshev unless the "circular distances" option is on, when it is Euclidean
    // (`line.h:194-204`). The pair (3,4) is the smallest one that shows the difference.
    assert_eq!(square_dist((0, 0), (3, 4)), 4, "the larger offset");
    assert_close(rl_dist((0, 0), (3, 4), false), 4.0, "Chebyshev by default");
    assert_close(
        rl_dist((0, 0), (3, 4), true),
        5.0,
        "and five with circles on",
    );
    // The asymmetry the header describes: a square three across and four up is inside a range of four by one metric and outside by the other.
    assert_eq!(square_dist((0, 0), (3, 4)), 4);
    assert!(rl_dist((0, 0), (3, 4), true) > 4.0);
}
