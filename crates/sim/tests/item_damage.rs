//! How damaged an item is, in the reference's six levels.
//!
//! Transcribed from `itype::damage_level` (`src/itype.cpp:117-126`).

#![allow(clippy::panic, clippy::expect_used)]

use cdda_sim::damage_level;

const MAX: i32 = 4000;

#[test]
fn an_undamaged_item_is_level_zero() {
    assert_eq!(damage_level(0, MAX, false), 0);
    assert_eq!(damage_level(0, MAX, true), 0, "even a stack");
}

#[test]
fn a_stack_is_ruined_by_any_damage_at_all() {
    assert_eq!(
        damage_level(1, MAX, true),
        5,
        "charges have no partial damage"
    );
    assert_eq!(damage_level(3999, MAX, true), 5);
}

#[test]
fn a_whole_item_climbs_the_scale_in_quarters() {
    assert_eq!(damage_level(MAX / 4, MAX, false), 2, "a quarter of the way");
    assert_eq!(damage_level(MAX / 2, MAX, false), 3, "half");
    assert_eq!(damage_level(MAX, MAX, false), 5, "the whole way");
}

#[test]
fn the_scale_saturates_rather_than_overflowing() {
    assert_eq!(
        damage_level(MAX * 2, MAX, false),
        5,
        "past the end is still level five"
    );
}

#[test]
fn a_type_with_no_damage_capacity_is_ruined_by_any_damage() {
    assert_eq!(damage_level(1, 0, false), 5, "nothing to scale against");
}
