//! The arithmetic of a fire that is not burning anything but itself.
//!
//! Reference: `field_processor_fd_fire` (`src/map_field.cpp:1222-1257`, `:1122-1131`, `:1308`). Four small rules decide whether a fire grows,
//! how fast it dies when it has nothing to eat, and how eagerly it jumps to a neighbour. None needs items or terrain, so all four can be
//! ported and pinned now - and each is a number a world's fire spread depends on.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::fire::{dying_out_cost, growth_cap, growth_cost, spread_chance};

#[test]
fn a_lone_fire_can_only_reach_intensity_one() {
    // With no neighbours and little fuel the cap is one: a fire that has burned for less than fifty minutes does not grow by itself.
    assert_eq!(growth_cap(0, 0), 1);
    assert_eq!(growth_cap(-10, 2), 1, "two neighbours is not three");
}

#[test]
fn enough_fuel_alone_lets_a_fire_grow_to_two() {
    // Two comments in the reference disagree with its own code and the code is the rule: it says "1 for a lone fire, 2 for at least 1
    // neighbor, 3 for at least 2 neighbors" where it checks **three** and **seven**, and its worked example calls fifty minutes of fuel
    // "2 (medium)" where the test is `age < -50` - strictly less, so fifty minutes exactly is not enough. Both are written here as the
    // code has them.
    assert_eq!(
        growth_cap(-51, 0),
        2,
        "fifty-one minutes of fuel on its own"
    );
    assert_eq!(growth_cap(-50, 0), 1, "and fifty exactly is not");
    assert_eq!(growth_cap(0, 3), 2, "three neighbours reach two");
    assert_eq!(growth_cap(0, 6), 2);
    assert_eq!(growth_cap(0, 7), 3, "seven neighbours reach three");
}

#[test]
fn a_raging_fire_needs_more_than_five_hundred_minutes_of_fuel() {
    // Strict again: `age < -500` means five hundred exactly is merely a medium fire.
    assert_eq!(growth_cap(-501, 0), 3);
    assert_eq!(growth_cap(-500, 0), 2, "five hundred exactly is not enough");
    // And the fuel rule is checked first: past five hundred minutes nothing else matters.
    assert_eq!(growth_cap(-600, 1), 3);
}

#[test]
fn growing_costs_ten_minutes_per_level_of_the_new_intensity() {
    // `cur.set_field_age( age + 10_minutes * cur.get_field_intensity() )` is evaluated **after** the intensity rises, so growing to three
    // costs thirty minutes and not twenty.
    assert_eq!(growth_cost(2), 20, "growing to two costs twenty minutes");
    assert_eq!(growth_cost(3), 30, "growing to three costs thirty");
}

#[test]
fn a_fire_with_nothing_to_burn_dies_ten_seconds_per_level() {
    // `cur.mod_field_age( 10_seconds * cur.get_field_intensity() )` - a bigger fire drains faster, which is what stops a bonfire raging
    // forever over bare stone.
    assert_eq!(dying_out_cost(1), 10);
    assert_eq!(dying_out_cost(3), 30);
}

#[test]
fn a_fire_spreads_more_eagerly_the_bigger_it_is_and_eagerly_onto_a_web() {
    // `spread_chance = 25 * ( intensity - 1 )`, and a neighbouring web makes it `50 + chance / 2`: intensity one cannot spread at all,
    // intensity three reaches fifty, and a web in the way doubles-ish that.
    assert_eq!(
        spread_chance(1, false),
        0,
        "a single square of fire does not jump"
    );
    assert_eq!(spread_chance(2, false), 25);
    assert_eq!(spread_chance(3, false), 50);
    assert_eq!(
        spread_chance(2, true),
        62,
        "fifty plus half of twenty-five, truncated"
    );
    assert_eq!(spread_chance(3, true), 75);
}
