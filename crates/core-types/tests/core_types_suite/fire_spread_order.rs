//! The spread rule's dice order, which is not the obvious one.
//!
//! Reference: `map_field.cpp:1330-1345`. The condition is
//! `can_spread && rng( 1, 100 ) < spread_chance && ( ... flag && one_in( n ) ... )`, and C++ evaluates it left to right with short-circuit `&&`. So the
//! chance is drawn **before** the destination's flags are consulted, a fire that fails the chance never rolls for the ground, and a fire that passes rolls
//! only for the flags that are actually set.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::fire::{Neighbour, spread_fire};
use cdda_core_types::rng::{Recording, Rolls};

fn ground_here() -> Neighbour {
    Neighbour {
        has_fire: false,
        web: false,
        flammable: true,
        flammable_ash: false,
        flammable_hard: false,
    }
}

/// The eight squares: the first is flammable ground, the rest are already on fire, so the scan spends one skip die on each of them and nothing else.
fn ring() -> [Neighbour; 8] {
    let mut neighbours = [Neighbour {
        has_fire: true,
        ..ground_here()
    }; 8];
    neighbours[0] = ground_here();
    neighbours
}

/// Seven skip dice for the seven burning neighbours after the first.
fn burning_ring_dice() -> Vec<String> {
    vec!["int 0 3 1".to_string(); 7]
}

#[test]
fn a_passed_chance_rolls_for_the_ground_and_lights_it() {
    // Start at neighbour 7 so the scan begins at 0; skip die says "do not skip", power is two, the chance passes at three, and the ground takes on
    // `one_in( 2 )`. Five dice, in that order.
    let neighbours = ring();
    let mut lines = vec![
        "int 0 7 7".to_string(),
        "int 0 3 1".to_string(),
        "int 0 4 2".to_string(),
        "int 1 100 3".to_string(),
        "int 0 1 0".to_string(),
    ];
    lines.extend(burning_ring_dice());
    let mut rolls = Rolls::replaying(Recording::parse(&lines.join("\n")).expect("a recording"));
    let caught = spread_fire(&mut rolls, 2, true, &neighbours);
    assert_eq!(
        caught,
        vec![0],
        "the first neighbour, which the scan reached first"
    );
    assert_eq!(
        rolls.recording_remaining(),
        Some(0),
        "and the ground roll came last, as the reference does it"
    );
}

#[test]
fn a_failed_chance_never_rolls_for_the_ground() {
    // 90 is above the chance of 25, so the condition short-circuits before `FLAMMABLE && one_in( 2 )`: the recording has no fourth die and must have none.
    let neighbours = ring();
    let mut lines = vec![
        "int 0 7 7".to_string(),
        "int 0 3 1".to_string(),
        "int 0 4 2".to_string(),
        "int 1 100 90".to_string(),
    ];
    lines.extend(burning_ring_dice());
    let mut rolls = Rolls::replaying(Recording::parse(&lines.join("\n")).expect("a recording"));
    let caught = spread_fire(&mut rolls, 2, true, &neighbours);
    assert!(caught.is_empty());
    assert_eq!(rolls.recording_remaining(), Some(0));
}
