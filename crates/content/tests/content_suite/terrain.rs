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

#[test]
fn the_two_scent_flags_are_in_the_dump_and_are_not_the_same_flag() {
    // `NO_SCENT` blocks scent outright and `REDUCE_SCENT` only slows it (`mapdata.h:181-193`), and both are terrain flags in the data rather than code rules. The counts are
    // pinned rather than the ids: 167 pieces of terrain block scent in this corpus and 225 reduce it, so a loader that dropped either flag would show here.
    //
    // **The dump's 245 is not the terrain's 225.** Twenty of the entries carrying `REDUCE_SCENT` are furniture (`f_*`), and this loader reads the terrain kind only, so a
    // test written against the whole dump disagrees with the loader while both are right - the split was found by exactly that mistake.
    let prototypes = dump();
    let blocks = prototypes
        .iter()
        .filter(|terrain| terrain.flags.iter().any(|flag| flag == "NO_SCENT"))
        .count();
    let reduces = prototypes
        .iter()
        .filter(|terrain| terrain.flags.iter().any(|flag| flag == "REDUCE_SCENT"))
        .count();
    assert_eq!(blocks, 167, "terrain that blocks scent");
    assert_eq!(reduces, 225, "terrain that reduces it");
    assert!(
        prototypes
            .iter()
            .filter(|terrain| terrain.flags.iter().any(|flag| flag == "NO_SCENT"))
            .all(|terrain| !terrain.flags.iter().any(|flag| flag == "REDUCE_SCENT")),
        "and no terrain carries both"
    );
}

#[test]
fn every_terrain_carries_the_symbol_it_is_drawn_with() {
    // Reference: `map_data_common_t::sym`, a `nc_color`-paired glyph (`mapdata.h`), which the draw path prints. The dump writes it as the four bytes of the UTF-8 sequence - a
    // floor is `[46, 46, 46, 46]`, four dots - because the reference stores it as a string of one grapheme rather than a `char`. The port keeps the first character, which is what
    // a cell prints, and a test pins that every terrain entry has one. The dump holds 1,769 entries of both kinds; this loader reads the 1,109 terrain ones, and furniture is
    // read by the same code from the same file.
    let prototypes = dump();
    assert_eq!(prototypes.len(), 1109);
    let missing: Vec<&str> = prototypes
        .iter()
        .filter(|terrain| terrain.symbol == ' ')
        .map(|terrain| terrain.id.as_str())
        .collect();
    assert!(
        missing.is_empty(),
        "terrains with no symbol: {}",
        missing.len()
    );
    let wall = terrain::find(&prototypes, "t_wall").expect("a wall");
    assert_eq!(wall.symbol, '#');
    let floor = terrain::find(&prototypes, "t_floor").expect("a floor");
    assert_eq!(floor.symbol, '.');
}
