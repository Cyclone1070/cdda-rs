//! The arithmetic of a fire that is not burning anything but itself.
//!
//! Reference: `field_processor_fd_fire` (`src/map_field.cpp:1222-1257`, `:1122-1131`, `:1308`). Four small rules decide whether a fire grows,
//! how fast it dies when it has nothing to eat, and how eagerly it jumps to a neighbour. None needs items or terrain, so all four can be
//! ported and pinned now - and each is a number a world's fire spread depends on.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::fire::{dying_out_cost, fire_turn, growth_cap, growth_cost, spread_chance};

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
    // costs thirty minutes and not twenty. The port counts in **turns**, and a turn is a second: ten minutes is six hundred of them.
    assert_eq!(
        growth_cost(2),
        1_200,
        "growing to two costs twenty minutes, which is twelve hundred turns"
    );
    assert_eq!(growth_cost(3), 1_800, "growing to three costs thirty");
}

#[test]
fn a_fire_with_nothing_to_burn_dies_ten_seconds_per_level() {
    // `cur.mod_field_age( 10_seconds * cur.get_field_intensity() )` - **ten seconds**, not ten minutes. My first version of this function
    // documented it as minutes while returning the second count, a factor of sixty out; the test now states the unit and the number.
    assert_eq!(dying_out_cost(1), 10, "ten seconds, which is ten turns");
    assert_eq!(dying_out_cost(3), 30);
    assert!(
        dying_out_cost(3) * 60 <= growth_cost(3),
        "dying is far slower than growing spends"
    );
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

#[test]
fn a_fire_with_fuel_and_nowhere_to_grow_stays_as_it_is() {
    // Age is fuel, and fuel alone is not enough: with no neighbours the cap is one, so a fire already at one cannot grow no matter how
    // much it has left. It keeps the fuel it had.
    // Ten turns of fuel: under fifty minutes, so the cap is one and a fire at one stays there. My first version of this test used ten
    // thousand turns, which is over five hundred minutes and reaches three on fuel alone - the case below, not this one.
    let turn = fire_turn(-10, 1, 0, true, 0);
    assert!(!turn.grew);
    assert_eq!(turn.intensity, 1);
}

#[test]
fn a_fire_with_fuel_and_neighbours_grows_and_pays_for_it() {
    // Seven neighbours make the cap three, so a fire at one with fuel climbs a level and spends twenty minutes doing it.
    let turn = fire_turn(-10_000, 1, 7, true, 0);
    assert!(turn.grew);
    assert_eq!(turn.intensity, 2);
    // Twenty minutes paid to grow - and ten turns lost first, because a fire standing on something that can burn ages by ten seconds a
    // level when it consumed nothing this turn. I had written the growth cost alone and the code charged both, which is the reference's
    // order: the fuel accounting comes before the growth.
    assert_eq!(
        turn.age,
        -10_000 + 10 + 1_200,
        "ten turns lost, then twenty minutes paid"
    );
}

#[test]
fn burning_fuel_first_lets_a_fire_grow_in_the_same_turn() {
    // The order matters: what the fire ate this turn is credited *before* the growth test, so a fire that consumed enough becomes hot
    // enough in the same turn rather than the next one.
    let cold = fire_turn(-10, 2, 0, true, 0);
    assert!(!cold.grew, "ten turns of fuel is not fifty minutes");
    let fed = fire_turn(-10, 2, 0, true, 3_600);
    assert!(fed.grew, "an hour of fuel in one turn is plenty");
    assert_eq!(
        fed.intensity, 3,
        "and seven neighbours are not needed for three, the fuel alone does it"
    );
}

#[test]
fn a_fire_on_nothing_loses_ten_seconds_a_level() {
    let turn = fire_turn(0, 2, 0, true, 0);
    assert_eq!(
        turn.age, 20,
        "twenty turns older, one per level per ten seconds"
    );
    assert!(!turn.grew);
    let bare = fire_turn(0, 2, 0, false, 0);
    assert_eq!(
        bare.age, 0,
        "on stone that cannot burn it does not age that way"
    );
}
