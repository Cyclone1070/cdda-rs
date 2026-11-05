//! Noise through a floor.
//!
//! Reference: `sound_distance` (`src/sounds.cpp:253-269`). The horizontal distance is the ordinary one; the vertical part is scaled five
//! times, and underground it is worse still - the reference's comment says 5x and 100x, and its arithmetic is stranger than the comment:
//! the first level below adds four to the attenuation *before* the five-times scaling, and every further level adds twenty more. A port
//! that took the comment at face value would be wrong in the second digit everywhere below ground.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::sound::sound_distance_3d;

#[test]
fn the_same_level_is_the_horizontal_distance() {
    // `rl_dist` over the corners: a diagonal is one step, not two.
    assert_eq!(sound_distance_3d((0, 0, 0), (3, 4, 0)), 4);
    assert_eq!(sound_distance_3d((0, 0, 0), (0, 5, 0)), 5);
    assert_eq!(sound_distance_3d((0, 0, 0), (0, 0, 0)), 0);
}

#[test]
fn one_level_above_ground_costs_five() {
    assert_eq!(
        sound_distance_3d((0, 0, 0), (0, 0, 1)),
        5,
        "the vertical distance scaled by five"
    );
    assert_eq!(
        sound_distance_3d((0, 0, 1), (0, 0, 0)),
        5,
        "and it does not matter which end is up"
    );
    assert_eq!(
        sound_distance_3d((0, 0, 0), (2, 0, 1)),
        7,
        "the horizontal part is added, not multiplied"
    );
}

#[test]
fn one_level_underground_costs_twenty_five() {
    // Lower end at z = -1 and one level of displacement: `dz + 4` is five, and the scaling makes it twenty-five.
    assert_eq!(sound_distance_3d((0, 0, 0), (0, 0, -1)), 25);
    assert_eq!(
        sound_distance_3d((0, 0, -1), (0, 0, -1)),
        0,
        "same square, same level"
    );
}

#[test]
fn deeper_displacement_adds_twenty_per_extra_level_before_scaling() {
    // The bonus is `( min( -lower_z, displacement ) - 1 ) * 20`, so two levels between -1 and 1 give `2 + 4 + 0` and three levels between
    // -2 and 1 give `3 + 4 + 20` - both then multiplied by five.
    assert_eq!(sound_distance_3d((0, 0, -1), (0, 0, 1)), 30);
    assert_eq!(sound_distance_3d((0, 0, -2), (0, 0, 1)), 135);
    assert_eq!(sound_distance_3d((0, 0, -3), (0, 0, 0)), 235);
}

#[test]
fn a_noise_below_ground_through_many_levels_is_heavily_attenuated() {
    // The rule grows without bound, which is the point: a monster on the surface should not hear a whisper three floors down.
    let near = sound_distance_3d((0, 0, 0), (0, 0, -1));
    let far = sound_distance_3d((0, 0, 0), (0, 0, -4));
    assert!(far > near * 4, "near {near}, far {far}");
}
