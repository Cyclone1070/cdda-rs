//! A fire that may not burn its ground spends no die at all.
//!
//! Reference: `map_field.cpp:1075-1100`. The whole fuel block is inside `if( can_burn )`, and `can_burn` is
//! `( ter.is_flammable() || frn.is_flammable() ) && !FIRE_CONTAINER`. So a fire on a tiled floor with a locker on it - the terrain is `FLAMMABLE_HARD` and the
//! furniture is neither flammable nor a container - draws **nothing**: no `one_in( 3 )`, no bash roll. The recording caught this as a missing die: the port
//! asked for fuel where the reference went straight on to the wind.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::fire::{Ground, ground_fuel};
use cdda_core_types::rng::{Recording, Rolls};

#[test]
fn a_ground_that_may_not_burn_draws_nothing() {
    // An empty recording: if the port asks for a die here it panics, which is the assertion.
    let mut rolls = Rolls::replaying(Recording::parse("").expect("an empty recording"));
    let fuel = ground_fuel(Ground::FlammableHard, 2, false, 0, &mut rolls);
    assert_eq!(
        fuel.time_added, 0,
        "a fire that cannot burn its ground is not fed"
    );
    assert!(!fuel.bashed, "and it does not break what it is standing on");
    assert_eq!(
        rolls.recording_remaining(),
        Some(0),
        "and it asked for nothing"
    );
}

#[test]
fn a_ground_that_may_burn_still_draws_its_die() {
    // `FLAMMABLE_HARD` is the branch with a die: `one_in( 3 )`, then the bash roll because the fire is above intensity one.
    let mut rolls =
        Rolls::replaying(Recording::parse("int 0 2 0\nint 0 99 11").expect("a recording"));
    let fuel = ground_fuel(Ground::FlammableHard, 2, true, 0, &mut rolls);
    assert_eq!(fuel.time_added, 2, "four minus the intensity of two");
    assert!(
        !fuel.bashed,
        "and `one_in( 100 )` only breaks the ground on a zero, which eleven is not"
    );
    assert_eq!(rolls.recording_remaining(), Some(0));
}
