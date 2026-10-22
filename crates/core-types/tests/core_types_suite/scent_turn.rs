//! One turn of the scent map, in the order the reference takes it.
//!
//! Reference: `do_turn.cpp:642-648`. Each turn, unless the character carries the `NO_SCENT` flag, they **deposit** their own scent at
//! their own square and the overmap records it; then the map **updates** - a diffusion pass over the window. The order matters: the
//! deposit is what the pass then spreads, so a port that diffused first would spread last turn's scent and never this turn's.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::scent::{ScentClock, scent_turn, scent_window};

#[test]
fn a_character_lays_their_scent_where_they_stand_and_the_pass_spreads_it() {
    let mut window = scent_window((40, 40));
    let mut clock = ScentClock::default();
    assert!(
        scent_turn(&mut window, &mut clock, (40, 40), 500, false, 0),
        "the turn updated the map"
    );
    // The deposit is at the square, the pass has already spread it, so the square holds less than what was deposited and its
    // neighbours hold some.
    let here = window.get(40, 40);
    assert!(
        here > 0 && here < 500,
        "the square keeps part of the deposit: {here}"
    );
    assert!(window.get(41, 40) > 0, "and its neighbours take some");
}

#[test]
fn a_character_who_does_not_smell_deposits_nothing_but_the_map_still_updates() {
    // The `NO_SCENT` flag is a debug mutation, and the reference puts the whole deposit behind it rather than behind the update.
    let mut window = scent_window((40, 40));
    let mut clock = ScentClock::default();
    assert!(
        scent_turn(&mut window, &mut clock, (40, 40), 500, true, 0),
        "the map still updates"
    );
    assert_eq!(window.get(40, 40), 0, "nothing was deposited");
    assert_eq!(window.get(41, 40), 0);
}

#[test]
fn the_turn_stops_updating_once_the_character_has_stood_still_long_enough() {
    // The pause is the update's, not the deposit's: the square still gets the character's scent, and nothing is spread.
    let mut window = scent_window((40, 40));
    let mut clock = ScentClock::default();
    assert!(scent_turn(&mut window, &mut clock, (40, 40), 500, false, 0));
    let neighbour_before = window.get(41, 40);
    assert!(neighbour_before > 0, "the first turn spread something");
    assert!(
        !scent_turn(&mut window, &mut clock, (40, 40), 500, false, 2_000),
        "the pause has run out"
    );
    assert_eq!(window.get(40, 40), 500, "the deposit is there, unspread");
    // Nothing moved: the neighbour keeps exactly what the last pass left it. My first version of this test asserted zero here, which
    // would only be true if the window had been cleared - the pause stops the pass, it does not erase what is already spread.
    assert_eq!(window.get(41, 40), neighbour_before, "and nothing diffused");
}

#[test]
fn the_same_seed_of_turns_gives_the_same_map() {
    // No dice in any of this, which is the point: scent is arithmetic, so two runs from the same deposit agree exactly. A port that
    // reached for the dice here would shift every later roll in the world.
    let run = || {
        let mut window = scent_window((40, 40));
        let mut clock = ScentClock::default();
        for now in 0..20 {
            scent_turn(&mut window, &mut clock, (40, 40), 500, false, now);
        }
        (0..10)
            .map(|offset| window.get(38 + offset, 40))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
}
