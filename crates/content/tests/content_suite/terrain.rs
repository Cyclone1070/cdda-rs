//! Terrain, and which of it burns.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::terrain::{self, Terrain};
use cdda_json::parse;
use std::path::Path;

fn dump() -> Vec<Terrain> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/terrain.json");
    let text = std::fs::read_to_string(path).expect("the terrain dump exists");
    let value = parse(text.as_bytes()).expect("the dump is valid JSON");
    terrain::from_dump(&value).expect("the dump loads")
}

#[test]
fn the_port_finds_every_terrain_the_reference_defines() {
    assert_eq!(
        dump().len(),
        1109,
        "terrains, without the furniture the dump also carries"
    );
}

#[test]
fn flammability_comes_from_the_three_flags_the_reference_uses() {
    let all = dump();
    let flammable = all.iter().filter(|t| t.is_flammable()).count();
    assert_eq!(flammable, 431, "terrains that burn");
    let door = terrain::find(&all, "t_door_c").expect("a wooden door");
    assert!(door.is_flammable(), "a wooden door burns: {:?}", door.flags);
    let stone = all
        .iter()
        .find(|t| !t.flags.iter().any(|f| f.contains("FLAMMABLE")))
        .expect("something that does not burn");
    assert!(!stone.is_flammable(), "{} has no flammable flag", stone.id);
}

#[test]
fn a_flag_that_merely_mentions_fire_is_not_flammability() {
    // Lava has USABLE_FIRE and is not flammable; a port that grepped for "FIRE" would set it
    // alight. Flammability is those three flags and nothing else.
    let all = dump();
    let lava = terrain::find(&all, "t_lava").expect("lava exists");
    assert!(
        lava.flags.iter().any(|f| f == "USABLE_FIRE"),
        "lava mentions fire: {:?}",
        lava.flags
    );
    assert!(!lava.is_flammable(), "and does not burn");
}

#[test]
fn impassable_terrain_costs_no_moves_because_it_cannot_be_entered() {
    let all = dump();
    let wall = terrain::find(&all, "t_wall").expect("a wall");
    assert!(wall.is_impassable(), "a wall cannot be walked into");
    let floor = terrain::find(&all, "t_floor").expect("a floor");
    assert!(!floor.is_impassable() && floor.move_cost > 0, "a floor can");
}

#[test]
fn a_terrain_with_no_id_is_refused() {
    let value = parse(br#"{"prototypes":[{"move_cost":2}]}"#).expect("valid JSON");
    assert!(terrain::from_dump(&value).is_err());
}

#[test]
fn the_port_finds_the_transparent_terrains_the_reference_reports() {
    // The stored bool, not a guess from the flags: `transparent` is set by the factory and mutated by `set_flag`/`unset_flag`
    // (`mapdata.cpp:876-905`), so a child that names `flags: []` or deletes `TRANSPARENT` is opaque with no flag saying so. The oracle now
    // dumps the bool, and these are its counts.
    let all = dump();
    let transparent = all.iter().filter(|t| t.transparent).count();
    assert_eq!(transparent, 736, "terrains light passes");
    assert_eq!(all.len() - transparent, 373, "and terrains it does not");

    let window = terrain::find(&all, "t_window").expect("a window");
    assert!(window.transparent, "a window passes light");
    let wall = terrain::find(&all, "t_wall").expect("a wall");
    assert!(!wall.transparent, "a wall does not");
}
