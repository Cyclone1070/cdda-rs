//! A gas that blows into the wind needs luck, and the luck is drawn per blocked neighbour.
//!
//! Transcribed from `map::spread_gas` (`src/map_field.cpp:341-378`). The branch only runs when the gas is **not** sheltered and the wind is at least five, and then the
//! three squares facing the wind are each asked `x_in_y( 1, max( 2, windpower ) )` - one draw per blocked candidate, in the order the clockwise scan collected them.
//! A port that asked once for the whole branch, or asked in the wrong order, would spread the same gas differently under the same seed.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::fire::{Around, GasDirection, spread_gas};
use cdda_core_types::rng::{Recording, Rolls};

fn windy(sheltered: bool, windpower: i32, wind_direction: i32) -> Around {
    Around {
        below: false,
        beside: [true; 8],
        above: false,
        sheltered,
        windpower,
        wind_direction,
    }
}

#[test]
fn a_windless_turn_does_not_look_at_the_blockers_at_all() {
    // `sheltered || windpower < 5` takes the plain branch: one draw to pick among the eight, and no real draws.
    let recording = Recording::parse("int 1 96 5\nint 0 7 0\nint 0 7 0\nint 0 7 4\n").expect("four dice");
    let mut rolls = Rolls::replaying(recording);
    let answer = spread_gas(3, 5, 30, &windy(false, 4, 0), &mut rolls);
    assert!(matches!(answer.direction, GasDirection::Side(_)), "{answer:?}");
    assert_eq!(rolls.recording_remaining(), Some(0), "nothing else was drawn");
}

#[test]
fn every_blocked_neighbour_draws_its_own_real_in_scan_order() {
    // Wind out of the north (0 degrees) blocks the three squares facing it: north_east, east and south_east - indices 2, 4 and 7 in the world's neighbour order.
    // The scan starts at index 0, so it visits 1, 2, 3, 4, 5, 6, 7, 0 and asks a real draw at 2, then 4, then 7. All three fail, leaving five candidates.
    // The order is the contract: the chance, the scan's start, **whether it goes at all**, then one real per blocked square, then the pick.
    let recording = Recording::parse(
        "int 1 90 5\nint 0 7 0\nint 0 7 0\nreal 0 1 0.9\nreal 0 1 0.9\nreal 0 1 0.9\nint 0 4 2\n",
    )
    .expect("three reals and three ints");
    let mut rolls = Rolls::replaying(recording);
    let answer = spread_gas(3, 5, 30, &windy(false, 10, 0), &mut rolls);
    assert_eq!(
        answer.direction,
        GasDirection::Side(5),
        "the third of the five survivors: 1, 3, 5, 6, 0"
    );
    assert_eq!(rolls.recording_remaining(), Some(0), "every die was spent");
}

#[test]
fn a_blocked_neighbour_that_wins_its_draw_stays_a_candidate() {
    // The first blocked square (north_east) wins, so it is among the six survivors and the pick lands on it.
    let recording = Recording::parse(
        "int 1 90 5\nint 0 7 0\nint 0 7 0\nreal 0 1 0.05\nreal 0 1 0.9\nreal 0 1 0.9\nint 0 5 0\n",
    )
    .expect("three reals and three ints");
    let mut rolls = Rolls::replaying(recording);
    // One chance in ten to beat the wind, so a draw of 0.05 wins where 0.9 loses.
    let answer = spread_gas(3, 5, 100, &windy(false, 10, 0), &mut rolls);
    assert_eq!(answer.direction, GasDirection::Side(1), "survivors: 1, 2, 3, 5, 6, 0");
}

#[test]
fn when_every_candidate_was_blocked_and_lost_the_gas_stays_put_and_does_not_rise() {
    // The reference's `else if` matters here: the lateral attempt was **taken** (`one_in` succeeded) and found nowhere to go, so the rising branch is not reached.
    // With the wind at 240 degrees the three squares facing it are south (6), south west (5) and south east (7), and here those are the only squares that could take
    // the gas - so every candidate is blocked. A port that fell through to the rise would move a gas the reference left alone.
    let recording = Recording::parse(
        "int 1 90 5\nint 0 7 0\nint 0 2 0\nreal 0 1 0.9\nreal 0 1 0.9\nreal 0 1 0.9\n",
    )
    .expect("three reals and three ints");
    let mut rolls = Rolls::replaying(recording);
    let mut around = windy(false, 10, 240);
    // Only the three squares the wind faces can take the gas, and each of them has to win its draw.
    around.beside = [false, false, false, false, false, true, true, true];
    let answer = spread_gas(3, 5, 100, &around, &mut rolls);
    assert_eq!(answer.direction, GasDirection::None, "{answer:?}");
    assert_eq!(rolls.recording_remaining(), Some(0), "every die was spent");
}
