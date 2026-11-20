//! **One map may build several overmap terrains.** `om_terrain` is a string, a list, or a grid of ids - the reference's `mapgen_factory` files the same map under every
//! id it names (`mapgen.cpp`, `mapgen_factory::add_terrain`), and the data relies on it: `nether_glass_deep`, `mansion_+4d` and `barry_mi-go_scout_tower_1` are all written
//! as **one-element lists**, not strings.

#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use cdda_content::mapgen;

fn objects(text: &str) -> Vec<mapgen::MapgenObject> {
    let value = cdda_json::parse(text.as_bytes()).expect("json");
    mapgen::load_objects(&value)
}

/// **A list of ids files the map under every one of them**, which is what makes a mapgen searchable by any of its names.
#[test]
fn a_list_of_terrains_files_the_map_under_each_one() {
    let found = objects(
        r#"{"type": "mapgen", "method": "json", "om_terrain": ["nether_glass_deep", "nether_glass_shallow"],
            "object": {"rows": ["..", ".."], "terrain": {".": "t_nether_glass"}}}"#,
    );
    assert_eq!(
        found.len(),
        2,
        "**one object per id**, so a search by either name finds the map"
    );
    assert_eq!(found[0].om_terrain, "nether_glass_deep");
    assert_eq!(found[1].om_terrain, "nether_glass_shallow");
    assert_eq!(
        found[1].rows.len(),
        2,
        "and both carry the same rows, because it is one map"
    );
}

/// **A grid of ids files the map under every column's id too**, which is the multi-tile form.
#[test]
fn a_grid_of_terrains_files_the_map_under_each_cell() {
    let found = objects(
        r#"{"type": "mapgen", "method": "json", "om_terrain": [["house_north", "house_east"]],
            "object": {"rows": ["."], "terrain": {".": "t_floor"}}}"#,
    );
    let names: Vec<&str> = found.iter().map(|one| one.om_terrain.as_str()).collect();
    assert_eq!(
        names,
        vec!["house_north", "house_east"],
        "**each id in the grid** is a terrain this map builds"
    );
}

/// **A mapgen's `fill_ter` names the terrain its chunk is filled with**, and a mapgen without one says so - which is the difference between a map that paves its whole
/// chunk and an update that must not touch the rest of the tile (`mapgen.cpp:5168-5172`).
#[test]
fn a_mapgen_carries_the_terrain_it_fills_its_chunk_with() {
    let found = objects(
        r#"{"type": "mapgen", "method": "json", "om_terrain": "house",
            "object": {"rows": ["."], "terrain": {".": "t_floor"}, "fill_ter": "t_rock_floor"}}"#,
    );
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].fill_ter.as_deref(),
        Some("t_rock_floor"),
        "**the fill is read**"
    );
    let bare = objects(
        r#"{"type": "mapgen", "method": "json", "om_terrain": "house",
            "object": {"rows": ["."], "terrain": {".": "t_floor"}}}"#,
    );
    assert_eq!(bare[0].fill_ter, None, "and a mapgen without one says so");
}

/// **A single string still reads as one**, so the shapes agree.
#[test]
fn one_terrain_is_still_one() {
    let found = objects(
        r#"{"type": "mapgen", "method": "json", "om_terrain": "shelter",
            "object": {"rows": ["."], "terrain": {".": "t_floor"}}}"#,
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].om_terrain, "shelter");
}
