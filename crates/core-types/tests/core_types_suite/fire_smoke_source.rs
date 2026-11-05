//! Smoke comes from the fuel, not from the map.
//!
//! Reference: `map_field.cpp:1063-1096`. Every fuel branch adds the same two lines:
//!
//! ```text
//! smoke += 2;
//! smoke += static_cast<int>( windpower / 5 );
//! ```
//!
//! so `smoke` is a **local of this turn's fuel block**, and it is only ever non-zero when a branch fed. The smoke gate twenty lines later reads that local
//! (`rng( 0, 100 - windpower ) <= smoke`), which means a fire that did not feed this turn is very unlikely to smoke, and a fire on ground that cannot burn
//! at all is exactly as unlikely as a bare field would be.
//!
//! The port used to ask a map method for this number, and every map it ever asked answered zero. The recording disagreed within one processor: the reference
//! drew `int 3 35` where the port went straight on to the hot air.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::fire::{Ground, ground_fuel};
use cdda_core_types::rng::{Recording, Rolls};

fn rolls(text: &str) -> Rolls {
    Rolls::replaying(Recording::parse(text).expect("a recording"))
}

#[test]
fn ground_that_feeds_produces_two_smoke() {
    let mut dice = Rolls::new(1);
    let fuel = ground_fuel(Ground::Flammable, 2, true, 0, &mut dice);
    assert_eq!(fuel.smoke, 2, "the two lines every fuel branch ends with");
}

#[test]
fn wind_adds_a_fifth_of_itself_to_the_smoke() {
    let mut dice = Rolls::new(1);
    let fuel = ground_fuel(Ground::Flammable, 2, true, 13, &mut dice);
    assert_eq!(
        fuel.smoke, 4,
        "two plus thirteen divided by five, rounded down"
    );
}

#[test]
fn hard_ground_that_refuses_produces_no_smoke_at_all() {
    // The whole branch is `one_in( 3 )`, and smoke is inside it: a turn the ground did not feed is a turn the fire does not smoke.
    let mut dice = rolls("int 0 2 1");
    let fuel = ground_fuel(Ground::FlammableHard, 2, true, 0, &mut dice);
    assert_eq!(fuel.smoke, 0);
    assert_eq!(dice.recording_remaining(), Some(0));
}

#[test]
fn a_fire_that_may_not_burn_its_ground_smokes_nothing() {
    let mut dice = Rolls::new(1);
    let fuel = ground_fuel(Ground::Flammable, 2, false, 0, &mut dice);
    assert_eq!(fuel.smoke, 0, "the fuel block never ran");
}

#[test]
fn inert_ground_smokes_nothing_even_though_the_fire_may_burn() {
    let mut dice = Rolls::new(1);
    let fuel = ground_fuel(Ground::Inert, 2, true, 0, &mut dice);
    assert_eq!(fuel.smoke, 0);
}
