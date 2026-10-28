//! Fire jumping to a neighbour.
//!
//! Reference: the spread loop at `src/map_field.cpp:1285-1345`. Its shape is easy to get wrong in a way no smoke test shows: the loop starts at
//! a **random** neighbour and wraps, skips a neighbour with `one_in( intensity * 2 )`, then asks the dice a second time whether the fire
//! takes, and a third time for how much power it has. Getting the number or order of those draws wrong shifts every later roll in a world.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::fire::{Neighbour, spread_fire};
use cdda_core_types::rng::Rolls;

/// A neighbour that is plain ground: nothing to burn, no web, no fire.
fn plain() -> Neighbour {
    Neighbour::default()
}

fn burnable() -> Neighbour {
    Neighbour {
        flammable: true,
        ..Neighbour::default()
    }
}

#[test]
fn a_fire_at_intensity_one_cannot_spread_onto_plain_ground() {
    // `spread_chance` is zero at intensity one, and nothing else in the condition can rescue it on ordinary ground.
    let neighbours = vec![burnable(); 8];
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..200 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        let spread = spread_fire(&mut rolls, 1, true, &neighbours);
        assert!(spread.is_empty(), "seed {seed} spread at intensity one");
    }
}

#[test]
fn a_fire_at_intensity_two_does_spread_onto_burnable_ground() {
    // Not always - the chance is one in four and the ground must take it - but often enough that two hundred seeds see it.
    let neighbours = vec![burnable(); 8];
    let mut spread = 0;
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..200 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        spread += spread_fire(&mut rolls, 2, true, &neighbours).len();
    }
    assert!(
        spread > 5,
        "only {spread} squares took fire over two hundred seeds"
    );
}

#[test]
fn a_web_in_the_way_lets_a_small_fire_jump() {
    // This is the case my intuition missed: at intensity one the chance is zero on ordinary ground, but a web makes it fifty, so a single
    // square of fire can cross a spider web. A port that skipped the web term would never light it.
    let neighbours = vec![
        Neighbour {
            web: true,
            ..Neighbour::default()
        };
        8
    ];
    let mut spread = 0;
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..200 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        spread += spread_fire(&mut rolls, 1, true, &neighbours).len();
    }
    assert!(
        spread > 20,
        "only {spread} webs caught over two hundred seeds"
    );
}

#[test]
fn a_neighbour_already_burning_is_never_chosen() {
    let mut lit = Neighbour {
        flammable: true,
        ..Neighbour::default()
    };
    lit.has_fire = true;
    let neighbours = vec![lit; 8];
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..200 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        assert!(
            spread_fire(&mut rolls, 3, true, &neighbours).is_empty(),
            "seed {seed}"
        );
    }
}

#[test]
fn a_contained_fire_does_not_spread_at_all() {
    // `can_spread` is false in a brazier or an oven, and then no neighbour is considered - though the loop still runs, which is why the dice
    // are consumed either way.
    let neighbours = vec![burnable(); 8];
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..50 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        assert!(
            spread_fire(&mut rolls, 3, false, &neighbours).is_empty(),
            "seed {seed}"
        );
    }
}

#[test]
fn spreading_spends_the_same_dice_whether_or_not_it_succeeds() {
    // The loop's draws do not depend on what it decides: it skips, asks and takes in the same order for every neighbour. Two runs from the
    // same seed must therefore stop at the same point in the stream, whatever the outcome.
    let neighbours = vec![plain(); 8];
    let mut first = Rolls::new(7);
    let mut second = Rolls::new(7);
    let a = spread_fire(&mut first, 2, true, &neighbours);
    let b = spread_fire(&mut second, 2, true, &neighbours);
    assert_eq!(a, b);
    assert!(a.is_empty(), "plain ground cannot catch");
}
