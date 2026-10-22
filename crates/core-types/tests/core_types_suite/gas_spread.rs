//! A gas moving to a neighbour, with the dice spelled out.
//!
//! Reference: `map::spread_gas` (`src/map_field.cpp:276-360`). The rule is shared by every gas - hot air, smoke, tear gas - because being a gas is
//! `percent_spread` in the data rather than a processor of its own. The order of its five dice is the rule, and each of the first two is a
//! **short-circuit**: intensity one and a failed gate both return before the neighbour scan.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::fire::spread_gas;
use cdda_core_types::rng::{Recording, Rolls};

fn replay(lines: &[&str]) -> Rolls {
    Rolls::replaying(Recording::parse(&lines.join("\n")).expect("a recording"))
}

const NONE: [bool; 8] = [false; 8];

#[test]
fn a_gas_at_intensity_one_spends_no_dice() {
    // `current_intensity <= 1 || rng(...)`: the short-circuit is the world. A dying gas must not shift the stream for everything after it.
    let mut rolls = replay(&["int 1 100 5"]);
    assert_eq!(spread_gas(1, 60, 10, 0, &NONE, &mut rolls), None);
    assert_eq!(
        rolls.recording_remaining(),
        Some(1),
        "the gate die was not drawn"
    );
}

#[test]
fn a_failed_gate_stops_before_looking_at_any_neighbour() {
    // Wind zero, ten percent spread: a gate of eleven fails, and nothing else is drawn.
    let mut rolls = replay(&["int 1 100 11"]);
    let can_take = [true; 8];
    assert_eq!(spread_gas(2, 60, 10, 0, &can_take, &mut rolls), None);
    assert_eq!(rolls.recording_remaining(), Some(0));
}

#[test]
fn a_passed_gate_scans_from_a_random_neighbour_and_takes_one() {
    // Gate 10 passes (it only fails when greater than ten), the scan starts at neighbour 3, one candidate qualifies, one_in(1) is always true and
    // the choice of one candidate is rng(0, 0), which costs a die and must return that candidate.
    let mut rolls = replay(&["int 1 100 10", "int 0 7 3", "int 0 0 0"]);
    let mut can_take = [false; 8];
    can_take[5] = true;
    let movement = spread_gas(3, 90, 10, 0, &can_take, &mut rolls).expect("the gas moves");
    assert_eq!(movement.to, 5, "the only square that would take it");
    assert_eq!(movement.source_intensity, 2, "the source keeps one less");
    assert_eq!(
        movement.transferred_age, 30,
        "a third of ninety turns goes with it"
    );
    assert_eq!(movement.source_age, 60);
    assert_eq!(rolls.recording_remaining(), Some(0));
}

#[test]
fn no_square_that_would_take_the_gas_means_no_move_and_no_more_dice() {
    // The scan draws nothing - `gas_can_spread_to` is a question about the map, not a roll - so an empty list ends the turn after two dice.
    let mut rolls = replay(&["int 1 100 1", "int 0 7 3"]);
    assert_eq!(spread_gas(2, 60, 10, 0, &NONE, &mut rolls), None);
    assert_eq!(rolls.recording_remaining(), Some(0));
}

#[test]
fn wind_narrows_the_gate_range_and_that_makes_spreading_easier() {
    // `rng( 1, 100 - windpower ) > percent_spread`: with a wind of ninety the range is 1..10, so a gate often passes where still air would have
    // refused it. The recording refuses a call with the wrong bounds, so asking for `rng( 1, 100 )` in wind fails this test rather than passing
    // quietly - which is the point of replaying dice instead of drawing them.
    let can_take = [true; 8];
    let mut still = replay(&["int 1 100 10", "int 0 7 0", "int 0 7 0", "int 0 7 0"]);
    assert!(spread_gas(2, 60, 10, 0, &can_take, &mut still).is_some());
    assert_eq!(still.recording_remaining(), Some(0));
    let mut windy = replay(&["int 1 10 10", "int 0 7 0", "int 0 7 0", "int 0 7 0"]);
    assert!(spread_gas(2, 60, 10, 90, &can_take, &mut windy).is_some());
    assert_eq!(windy.recording_remaining(), Some(0));
}
