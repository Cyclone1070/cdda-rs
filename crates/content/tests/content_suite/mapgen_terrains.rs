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

/// **A mapgen's `traps` symbols are read as ids**, the third layer a character can carry (`mapgen.cpp:5470-5480`).
#[test]
fn a_mapgen_carries_the_traps_its_symbols_set() {
    let found = objects(
        r#"{"type": "mapgen", "method": "json", "om_terrain": "field",
            "object": {"rows": ["T."], "terrain": {".": "t_dirt", "T": "t_dirt"},
                       "traps": {"T": "tr_bear"}}}"#,
    );
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].traps.get(&'T').map(String::as_str),
        Some("tr_bear"),
        "**the trap layer is read**"
    );
    let bare = objects(
        r#"{"type": "mapgen", "method": "json", "om_terrain": "field",
            "object": {"rows": ["."], "terrain": {".": "t_dirt"}}}"#,
    );
    assert!(
        bare[0].traps.is_empty(),
        "and a mapgen without traps says so"
    );
}

/// **A mapgen's `items` symbols are read as placements**: an item, a chance out of a hundred that defaults to certainty, and an amount that may be a number or a range
/// (`jmapgen_item_group::load`, `mapgen.cpp:5320-5340`). A bare string is an item **group**, which this layer does not read yet, and the loader leaves it out rather than
/// pretending it is an item.
#[test]
fn a_mapgen_carries_the_items_its_symbols_place() {
    let found = objects(
        r#"{"type": "mapgen", "method": "json", "om_terrain": "house",
            "object": {"rows": ["abc"], "terrain": {"a": "t_floor", "b": "t_floor", "c": "t_floor"},
                       "items": {"a": {"item": "rock", "chance": 70, "amount": [1, 3]},
                                 "b": {"item": "rock"},
                                 "c": "livingroom"}}}"#,
    );
    assert_eq!(found.len(), 1);
    let items = &found[0].items;
    let rock = items
        .get(&'a')
        .and_then(|placements| placements.first())
        .expect("the ranged one is read");
    assert_eq!(rock.item.as_deref(), Some("rock"));
    assert_eq!(rock.chance, 70, "**the chance is read**");
    assert_eq!(rock.amount, (1, 3), "and the range");
    let plain = items
        .get(&'b')
        .and_then(|placements| placements.first())
        .expect("the bare one is read");
    assert_eq!(
        plain.chance, 100,
        "**a placement with no chance is certain**"
    );
    assert_eq!(plain.amount, (1, 1), "and places one");
    assert_eq!(
        items
            .get(&'c')
            .and_then(|placements| placements.first())
            .and_then(|placement| placement.group.as_deref()),
        Some("livingroom"),
        "**a bare string is a group, and is read as one**"
    );
}

/// **A mapgen's `place_monster` entries are read as placements**: a monster, a chance out of a hundred that defaults to certainty, a pack size and a square range
/// (`jmapgen_monster::load`, `mapgen.cpp:5560-5610`).
#[test]
fn a_mapgen_carries_the_monsters_it_places() {
    let found = objects(
        r#"{"type": "mapgen", "method": "json", "om_terrain": "house",
            "object": {"rows": ["."], "terrain": {".": "t_floor"},
                       "place_monster": [{"monster": "mon_zombie", "x": [1, 4], "y": 3, "pack_size": [2, 5]}]}}"#,
    );
    assert_eq!(found.len(), 1);
    let zombie = &found[0].monsters[0];
    assert_eq!(zombie.monster.as_deref(), Some("mon_zombie"));
    assert_eq!(
        zombie.chance, 100,
        "**a placement with no chance is certain**"
    );
    assert_eq!(zombie.pack_size, (2, 5), "and the pack is a range");
    assert_eq!(zombie.x, (1, 4), "the columns are a range");
    assert_eq!(zombie.y, (3, 3), "and a single row is one row");
}

/// **A mapgen may name the terrain whose map is drawn under it** (`predecessor_mapgen`, `mapgen.cpp:5176-5180`), and a mapgen without one says so.
#[test]
fn a_mapgen_carries_its_predecessor() {
    let found = objects(
        r#"{"type": "mapgen", "method": "json", "om_terrain": "house_roof",
            "object": {"rows": ["."], "terrain": {".": "t_roof"}, "predecessor_mapgen": "house"}}"#,
    );
    assert_eq!(
        found[0].predecessor_mapgen.as_deref(),
        Some("house"),
        "**the predecessor is read**"
    );
    let bare = objects(
        r#"{"type": "mapgen", "method": "json", "om_terrain": "house_roof",
            "object": {"rows": ["."], "terrain": {".": "t_roof"}}}"#,
    );
    assert_eq!(bare[0].predecessor_mapgen, None, "and one without says so");
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
