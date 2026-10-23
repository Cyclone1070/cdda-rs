//! Whether a square will take a creature.
//!
//! Reference: `map::passable_through` (`map.cpp:2419-2422`) - `passable( p ) && ( has_floor( p ) || has_floor_or_support( p ) )`,
//! where `passable` is `move_cost != 0` (`map.cpp:2414-2417`) and `has_floor` is false for terrain flagged `NO_FLOOR`
//! (`map.cpp:2639-2647`). The two halves are separate questions and both have to hold: open air has room and no floor, a wall
//! has a floor and no room.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_content::mapdata::{MapData, passable_through};

fn terrain(move_cost: i32, flags: &[&str]) -> MapData {
    MapData {
        kind: cdda_content::mapdata::MapDataKind::Terrain,
        id: "t_probe".to_owned(),
        name: "probe".to_owned(),
        symbol: [0; 4],
        color: [cdda_core_types::colors::NcColor::default(); 4],
        move_cost,
        light: 0,
        coverage: 0,
        flags: flags.iter().map(|flag| (*flag).to_owned()).collect(),
        transparent: true,
    }
}

#[test]
fn a_wall_takes_nobody_and_a_floor_takes_anybody() {
    assert!(
        !passable_through(&terrain(0, &[])),
        "no move cost, no way in"
    );
    assert!(
        passable_through(&terrain(2, &[])),
        "a floor with a cost is passable"
    );
}

#[test]
fn open_air_has_room_but_no_floor() {
    // The half that is easy to miss: `NO_FLOOR` alone refuses the square even when the move cost says there is room. A port that
    // only looked at `move_cost` would let a spawned monster stand in mid-air.
    assert!(
        !passable_through(&terrain(2, &["NO_FLOOR"])),
        "no floor under it"
    );
    assert!(
        passable_through(&terrain(2, &["FLAT"])),
        "an unrelated flag changes nothing"
    );
}

#[test]
fn a_square_with_support_below_is_passable_even_without_its_own_floor() {
    // `has_floor_or_support` is the second half of the disjunction, and it is why terrain without a floor of its own can still
    // take a creature when something holds it up.
    assert!(passable_through_with_support(
        &terrain(2, &["NO_FLOOR"]),
        true
    ));
    assert!(!passable_through_with_support(
        &terrain(2, &["NO_FLOOR"]),
        false
    ));
}

/// The rule with the level below made explicit, which is what a multi-level map supplies.
fn passable_through_with_support(terrain: &MapData, support: bool) -> bool {
    cdda_content::mapdata::passable_through_with_support(terrain, support)
}
