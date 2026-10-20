//! What burning does to an item's charge count or its accumulated damage.
//!
//! Transcribed from `item::burn` (`src/item.cpp:12609-12643`) and `roll_remainder` (`src/rng.cpp`).

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::rng::Rolls;
use cdda_sim::{BurnState, burn_item, roll_remainder};

fn stack(charges: i32, volume_ml: f64) -> BurnState {
    BurnState {
        by_charges: true,
        charges,
        stack_size: 1,
        volume_ml,
        burnt: 0,
    }
}

fn solid(volume_ml: f64) -> BurnState {
    BurnState {
        by_charges: false,
        charges: 0,
        stack_size: 1,
        volume_ml,
        burnt: 0,
    }
}

#[test]
fn a_whole_number_rounds_without_asking_the_dice() {
    let mut rolls = Rolls::new(1);
    let mut control = Rolls::new(1);
    assert_eq!(roll_remainder(3.0, &mut rolls), 3);
    assert_eq!(
        rolls.rng(1, 1000),
        control.rng(1, 1000),
        "a whole number draws nothing"
    );
}

#[test]
fn a_fraction_rounds_up_sometimes() {
    let mut up = 0;
    let mut down = 0;
    for seed in 1..=100 {
        let mut rolls = Rolls::new(seed);
        if roll_remainder(2.5, &mut rolls) == 3 {
            up += 1;
        } else {
            down += 1;
        }
    }
    assert!(
        up > 20 && down > 20,
        "{up} up, {down} down over 100 seeds: a fair half"
    );
}

#[test]
fn a_stack_loses_charges_in_proportion_to_the_fire() {
    let mut state = stack(100, 250.0);
    let mut rolls = Rolls::new(1);
    // burn 0.03 * 250ml * 1 / (3 * 250ml) = 0.01 charges.
    let gone = burn_item(&mut state, 0.03, &mut rolls);
    assert!(!gone);
    assert!(
        state.charges >= 99,
        "charges {} : at most one lost",
        state.charges
    );
}

#[test]
fn a_stack_with_no_volume_is_destroyed_outright() {
    let mut state = stack(100, 0.0);
    let mut rolls = Rolls::new(1);
    let mut control = Rolls::new(1);
    assert!(
        burn_item(&mut state, 0.0, &mut rolls),
        "no volume means no saving it"
    );
    assert_eq!(state.charges, 0);
    assert_eq!(
        rolls.rng(1, 1000),
        control.rng(1, 1000),
        "and it draws nothing"
    );
}

#[test]
fn a_solid_item_burns_away_in_proportion_to_its_size() {
    // 500 ml is two measures of 250 ml, so it takes six points of burning.
    let mut state = solid(500.0);
    let mut rolls = Rolls::new(1);
    assert!(
        !burn_item(&mut state, 5.0, &mut rolls),
        "five of six is not gone"
    );
    assert_eq!(state.burnt, 5);
    let mut rolls = Rolls::new(1);
    assert!(
        burn_item(&mut state, 1.0, &mut rolls),
        "the sixth finishes it"
    );
}

#[test]
fn a_large_solid_takes_longer_than_a_small_one() {
    let mut small = solid(250.0);
    let mut large = solid(2000.0);
    let mut rolls = Rolls::new(1);
    assert!(
        burn_item(&mut small, 3.0, &mut rolls),
        "250 ml is three measures"
    );
    let mut rolls = Rolls::new(1);
    assert!(
        !burn_item(&mut large, 3.0, &mut rolls),
        "2000 ml is twenty-four"
    );
    assert_eq!(large.burnt, 3);
}
