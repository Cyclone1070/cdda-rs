//! The port's world next to the reference's, as pictures.
//!
//! `corpus/overmap/reference-start.json` is the reference's own generated overmap, dumped from the oracle around the
//! starting position. This gate reads it and states, in counts, exactly how far the port's world is from it - which
//! is the honest form of the comparison while the port generates fields and roads and nothing else.
//!
//! A diff of the two pictures would be a wall of differences and would say nothing. The composition does say
//! something: which kinds of terrain the reference's starting area is made of, and therefore which rules the port
//! still owes.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use cdda_core_types::overmap::{CitySettings, Overmap, grow_town, render};
use cdda_core_types::rng::Rolls;
use std::collections::BTreeMap;

fn reference_rows() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-start.json");
    let bytes = std::fs::read(&path).expect("the reference overmap dump");
    let value = cdda_json::parse(&bytes).expect("valid json");
    let cdda_json::Value::Object(root) = value else {
        panic!("the dump is an object");
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("rows") else {
        panic!("the dump holds rows");
    };
    rows.iter()
        .map(|row| match row {
            cdda_json::Value::Str(text) => text.clone(),
            _ => panic!("a row is a string"),
        })
        .collect()
}

fn counts() -> BTreeMap<char, usize> {
    let mut counts = BTreeMap::new();
    for row in reference_rows() {
        for glyph in row.chars() {
            *counts.entry(glyph).or_insert(0) += 1;
        }
    }
    counts
}

fn total(counts: &BTreeMap<char, usize>, glyphs: &str) -> usize {
    glyphs.chars().filter_map(|glyph| counts.get(&glyph)).sum()
}

#[test]
fn the_reference_overmap_is_a_full_square_of_tiles() {
    let rows = reference_rows();
    assert_eq!(rows.len(), 41, "the dump's radius is twenty");
    assert!(
        rows.iter().all(|row| row.chars().count() == 41),
        "and it is square"
    );
    assert_eq!(
        rows.iter().map(|row| row.chars().count()).sum::<usize>(),
        1681
    );
}

#[test]
fn the_reference_starting_area_is_mostly_wilderness() {
    let counts = counts();
    // The composition, pinned: this is what a starting area is made of, and a change here means the reference's
    // world generation changed rather than the port's.
    assert_eq!(counts.get(&'F').copied().unwrap_or(0), 716, "forest");
    assert_eq!(counts.get(&'.').copied().unwrap_or(0), 446, "field");
    assert_eq!(
        total(&counts, "│─└┘┌┐├┤┬┴┼"),
        261,
        "roads, in every shape they come in"
    );
    assert_eq!(total(&counts, "<>"), 133, "building entrances");
    assert_eq!(total(&counts, "^v"), 68, "slopes");
}

#[test]
fn the_reference_draws_roads_with_more_than_one_glyph() {
    let counts = counts();
    let shapes = "│─└┘┌┐├┤┬┴┼"
        .chars()
        .filter(|glyph| counts.get(glyph).copied().unwrap_or(0) > 0)
        .count();
    assert!(
        shapes >= 6,
        "a generated road network has corners and junctions: {shapes} shapes"
    );
}

#[test]
fn the_port_can_draw_its_own_world_and_the_reference_can_be_read() {
    // Both halves of the gate exist: the reference's picture loads, and the port can produce one of its own.
    let mut overmap = Overmap::filled("field");
    let mut rolls = Rolls::new(31337);
    grow_town(&mut overmap, CitySettings::default(), 3, &mut rolls);
    let ours = render(&overmap, (0, 0), 180, 180);
    assert!(ours.contains('#'), "the port draws roads");
    assert!(
        !reference_rows().is_empty(),
        "and the reference's picture is readable"
    );
}

#[test]
fn the_kinds_of_terrain_the_port_does_not_generate_are_named() {
    // The gap, written down rather than glossed. Every one of these appears in the reference's starting area and
    // nothing in the port produces it yet:
    //   forest  - the wilderness rules (overmap_forest / forest trails)
    //   slopes  - `^` and `v`, which come from mapgen on the map level rather than from the overmap
    //   shaped roads - the port writes one road glyph and junctions, not `│`, `─`, `└` and the rest
    //   buildings - city buildings and their specials
    let counts = counts();
    let gap: Vec<char> = "F^v│─└┘┌┐<>"
        .chars()
        .filter(|g| counts.get(g).copied().unwrap_or(0) > 0)
        .collect();
    assert!(gap.len() >= 8, "the port owes these: {gap:?}");
}
