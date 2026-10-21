//! How many charges fit in a space.
//!
//! Transcribed from `item::charges_per_volume` (`src/item.cpp:1391-1420`).

#![allow(clippy::panic, clippy::expect_used)]

use cdda_sim::charges_per_volume;

#[test]
fn a_charge_takes_its_share_of_the_stack_volume() {
    // A liquid whose stack is two charges per 500 ml: one charge is 250 ml, so 250 ml holds one.
    assert_eq!(charges_per_volume(250, 500, 2, true), 1);
    assert_eq!(charges_per_volume(1000, 500, 2, true), 4);
}

#[test]
fn a_plain_item_is_counted_whole() {
    // A 100 ml object in 250 ml of space: two fit, and the leftover is not a third.
    assert_eq!(charges_per_volume(250, 100, 1, false), 2);
    assert_eq!(charges_per_volume(99, 100, 1, false), 0, "not quite one");
}

#[test]
fn an_item_with_no_volume_is_unlimited() {
    assert_eq!(charges_per_volume(250, 0, 1, true), i32::MAX);
}

#[test]
fn no_space_holds_nothing() {
    assert_eq!(
        charges_per_volume(0, 250, 1, true),
        0,
        "zero space, zero charges"
    );
    assert_eq!(charges_per_volume(0, 250, 1, false), 0);
}
