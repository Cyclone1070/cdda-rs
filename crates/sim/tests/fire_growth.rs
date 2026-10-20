//! Fire growing into a bigger fire, and paying for it.
//!
//! Transcribed from `field_processor_fd_fire` (`src/map_field.cpp:1240-1262`). The ceiling comes
//! from fuel age first, then neighbours; growth costs ten minutes of age per point of intensity.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_sim::{grow_fire, growth_ceiling};

#[test]
fn half_an_hour_of_fuel_allows_a_raging_fire_on_its_own() {
    assert_eq!(growth_ceiling(-30_001, 0), 3, "just under -500 minutes");
    // At exactly -500 minutes the top rule does not apply, but the -50 minute rule still gives 2:
    // the two thresholds overlap, and the reference's order is what makes the answer 2 rather than 1.
    assert_eq!(
        growth_ceiling(-30_000, 0),
        2,
        "exactly -500 minutes falls through to the lower rule"
    );
}

#[test]
fn neighbours_raise_the_ceiling_when_fuel_age_does_not() {
    assert_eq!(growth_ceiling(-100, 0), 1, "alone and young");
    assert_eq!(growth_ceiling(-100, 2), 1, "two neighbours are not three");
    assert_eq!(growth_ceiling(-100, 3), 2, "three neighbours");
    assert_eq!(growth_ceiling(-100, 6), 2, "six are still not seven");
    assert_eq!(growth_ceiling(-100, 7), 3, "seven neighbours");
}

#[test]
fn fifty_minutes_of_fuel_raises_a_lone_fire_to_two() {
    assert_eq!(growth_ceiling(-3_001, 0), 2, "just under -50 minutes");
    assert_eq!(
        growth_ceiling(-3_000, 0),
        1,
        "exactly -50 minutes is not enough"
    );
    assert_eq!(growth_ceiling(-3_001, 3), 2, "neighbours already allow two");
}

#[test]
fn growing_costs_ten_minutes_per_point_of_the_new_size() {
    assert_eq!(
        grow_fire(1, -30_001, 0),
        Some((2, -30_001 + 1_200)),
        "one step, twenty minutes"
    );
    assert_eq!(
        grow_fire(2, -30_001, 0),
        Some((3, -30_001 + 1_800)),
        "two steps, half an hour"
    );
}

#[test]
fn a_fire_does_not_grow_past_its_ceiling_or_once_its_fuel_is_spent() {
    assert_eq!(grow_fire(1, -100, 0), None, "young and alone: ceiling is 1");
    assert_eq!(grow_fire(3, -30_001, 0), None, "already raging");
    assert_eq!(
        grow_fire(1, 0, 7),
        None,
        "no fuel age left, so no growth even with neighbours"
    );
    assert_eq!(grow_fire(1, 5, 7), None, "positive age never grows");
}
