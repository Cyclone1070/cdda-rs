//! A square with something burnable on it catches even when the ground will not.
//!
//! Reference: `map_field.cpp:1343`, the last disjunct of the spread condition:
//!
//! ```text
//! || ( one_in( 5 ) && dst.get_item_count() > 0 && flammable_items_at( ... ) )
//! ```
//!
//! C++ draws `one_in( 5 )` **before** it asks whether anything is there, so the die is spent on every square that gets that far, items or not. What the die
//! then decides is whether a square with a burnable thing on it catches - which is the only way a spreading fire gets onto a tiled floor from a fire that is
//! too weak to break it.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::fire::{Ground, Neighbour, spread_fire};
use cdda_core_types::rng::{Recording, Rolls};

/// The eight squares: the first is tiled floor with a plank on it, the rest are already on fire so the scan spends one skip die on each.
fn ring_with_a_plank() -> [Neighbour; 8] {
    let mut neighbours = [Neighbour {
        has_fire: true,
        ..Neighbour::default()
    }; 8];
    neighbours[0] = Neighbour {
        has_fire: false,
        flammable_hard: true,
        items: true,
        ..Neighbour::default()
    };
    neighbours
}

/// Seven skip dice for the seven burning neighbours after the first.
fn burning_ring() -> Vec<String> {
    vec!["int 0 3 1".to_string(); 7]
}

#[test]
fn a_plank_on_hard_ground_catches_on_the_item_die_alone() {
    let neighbours = ring_with_a_plank();
    let mut lines = vec![
        "int 0 7 7".to_string(), // the scan starts at the seventh neighbour, so it reaches the plank first
        "int 0 3 1".to_string(), // do not skip the first square
        "int 0 4 2".to_string(), // power stays two, which is under what hard ground needs, so no ground die is spent
        "int 1 100 3".to_string(), // the chance passes
        "int 0 4 0".to_string(), // the item die, which does find a plank
    ];
    lines.extend(burning_ring());
    let mut rolls = Rolls::replaying(Recording::parse(&lines.join("\n")).expect("a recording"));
    let caught = spread_fire(&mut rolls, 2, true, &neighbours);
    assert_eq!(
        caught,
        vec![0],
        "the plank catches where the floor would not"
    );
    assert_eq!(rolls.recording_remaining(), Some(0));
}

#[test]
fn the_item_die_is_spent_even_when_the_square_is_empty() {
    // Nothing lies there, so the die changes nothing - but it is still drawn, and a port that skips it is one draw behind for the rest of the turn.
    let mut neighbours = ring_with_a_plank();
    neighbours[0] = Neighbour {
        has_fire: false,
        flammable_hard: true,
        items: false,
        ..Neighbour::default()
    };
    let mut lines = vec![
        "int 0 7 7".to_string(),
        "int 0 3 1".to_string(),
        "int 0 4 2".to_string(),
        "int 1 100 3".to_string(),
        "int 0 4 0".to_string(), // the item die fires, and finds nothing, because nothing is there
    ];
    lines.extend(burning_ring());
    let mut rolls = Rolls::replaying(Recording::parse(&lines.join("\n")).expect("a recording"));
    let caught = spread_fire(&mut rolls, 2, true, &neighbours);
    assert!(caught.is_empty(), "an empty square catches nothing");
    assert_eq!(
        rolls.recording_remaining(),
        Some(0),
        "and the die went anyway"
    );
}

#[test]
fn the_ground_still_decides_first_when_it_can() {
    // Plain `FLAMMABLE` ground needs no die at all, so the item branch is never reached and the plank is irrelevant.
    let mut neighbours = ring_with_a_plank();
    neighbours[0] = Neighbour {
        has_fire: false,
        flammable: true,
        items: true,
        ..Neighbour::default()
    };
    let mut lines = vec![
        "int 0 7 7".to_string(),
        "int 0 3 1".to_string(),
        "int 0 4 2".to_string(),
        "int 1 100 3".to_string(),
        "int 0 1 0".to_string(), // plain FLAMMABLE ground takes on `one_in( 2 )`
    ];
    lines.extend(burning_ring());
    let mut rolls = Rolls::replaying(Recording::parse(&lines.join("\n")).expect("a recording"));
    let caught = spread_fire(&mut rolls, 2, true, &neighbours);
    assert_eq!(caught, vec![0]);
    assert_eq!(
        rolls.recording_remaining(),
        Some(0),
        "the ground caught it, so nothing else was asked"
    );
    let _ = Ground::Flammable;
}
