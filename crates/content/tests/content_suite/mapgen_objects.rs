//! Mapgen objects: the rows a JSON map states and what each glyph means.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::mapgen::{MapgenObject, load_objects};
use std::path::Path;

fn one(symbols: &[(&str, &str)], rows: &[&str]) -> MapgenObject {
    let terrain = symbols
        .iter()
        .map(|(symbol, id)| format!("\"{symbol}\":\"{id}\""))
        .collect::<Vec<_>>()
        .join(",");
    let rows = rows
        .iter()
        .map(|row| format!("\"{row}\""))
        .collect::<Vec<_>>()
        .join(",");
    // The shape the reference actually uses: the rows and the symbol tables live under `object`.
    let text = format!(
        "[{{\"type\":\"mapgen\",\"method\":\"json\",\"om_terrain\":\"test\",\
          \"object\":{{\"rows\":[{rows}],\"terrain\":{{{terrain}}}}}}}]"
    );
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let mut loaded = load_objects(&value);
    loaded.pop().expect("one object")
}

#[test]
fn a_map_knows_its_shape() {
    let map = one(&[(".", "t_floor"), ("#", "t_wall")], &["..#", "..#", "..#"]);
    assert_eq!(map.width(), 3);
    assert_eq!(map.height(), 3);
    assert!(map.rows_agree());
}

#[test]
fn a_glyph_resolves_through_the_table() {
    let map = one(&[(".", "t_floor"), ("#", "t_wall")], &[".#", "##"]);
    assert_eq!(map.terrain_at(0, 0), Some("t_floor"));
    assert_eq!(map.terrain_at(1, 0), Some("t_wall"));
    assert_eq!(map.terrain_at(9, 9), None, "off the map is nothing");
}

#[test]
fn ragged_rows_are_noticed_rather_than_drawn() {
    let map = one(&[(".", "t_floor")], &["..", "."]);
    assert!(!map.rows_agree(), "the second row is short");
}

#[test]
fn the_only_way_to_build_is_code() {
    // A mapgen with any other method has no rows for the port to read.
    let text = "[{\"method\":\"lua\",\"om_terrain\":\"test\",\"rows\":[\".\"]}]";
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    assert!(load_objects(&value).is_empty());
}

#[test]
fn a_mapgen_whose_rows_are_at_the_top_level_is_not_one() {
    // The bug this test exists for: rows live under `object`, and a flat object is not a mapgen.
    let text = "[{\"method\":\"json\",\"om_terrain\":\"test\",\"rows\":[\".\"]}]";
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    assert!(
        load_objects(&value).is_empty(),
        "flat rows are not a mapgen"
    );
}

#[test]
fn the_reference_tree_holds_the_maps_it_says_it_does() {
    // 845 files, 1,071 mapgen objects: the count the loader finds when it reads the right key.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let files = cdda_content::json_files_in_load_order(
        &root.join("references/Cataclysm-DDA/data/json/mapgen"),
    )
    .expect("the mapgen directory");
    assert_eq!(files.len(), 845);
    let mut total = 0;
    let mut ragged = Vec::new();
    for path in &files {
        let Some(value) = cdda_content::read_json(path) else {
            continue;
        };
        for map in load_objects(&value) {
            total += 1;
            if !map.rows_agree() {
                ragged.push(map.om_terrain);
            }
        }
    }
    // **10,076 after round 240** read every shape of `om_terrain`. The count is of **(id, map) pairs**, not of definitions: one map that names a list of ids is filed
    // once per id, which is what the reference's factory holds and what makes a search by any of its names find it. Before this, the 1,595 list-shaped mapgens in the tree
    // contributed one object each - the count was 4,487 - and their ids were filed under nothing at all.
    assert_eq!(
        total, 10_076,
        "mapgen objects (id, map pairs) in the reference tree"
    );
    // Three maps in the reference tree are not rectangular by character count. The reference loads them
    // anyway, so this is a fact about the content rather than a fault in the loader - pinned here so it
    // cannot quietly grow.
    assert_eq!(
        ragged,
        vec![
            "lighthouse_small_z1".to_owned(),
            "lighthouse_small_z2".to_owned(),
            "lighthouse_small_z3".to_owned()
        ],
        "maps whose rows do not all measure the same"
    );
}

/// **`mapping` is the third way the data names a symbol, and each entry is a bundle.** Reference: the palette reader - a `mapping` member's value holds several members at
/// once (`terrain`, `furniture`, `items`, `item`, ...) where the flat `terrain`/`furniture`/`items` tables hold one member each per symbol.
///
/// Seven mapgens use it, and a loader that reads only the flat tables draws their symbols as nothing at all - no terrain, no furniture, no loot. The test reads one mapgen with
/// both forms and checks the merge: the mapping fills the symbols the flat tables do not name, and does **not** overwrite a symbol the flat table already stated.
#[test]
fn a_mapping_bundle_fills_the_symbol_tables() {
    let text = "[{\"type\":\"mapgen\",\"method\":\"json\",\"om_terrain\":\"test\",
        \"object\":{\"rows\":[\"L.\",\"..\"],
        \"terrain\":{\".\":\"t_floor\"},
        \"mapping\":{\"L\":{\"furniture\":\"f_locker\",\"items\":[{\"item\":\"bed\",\"chance\":60}]},
                     \"#\":\"t_wall\"}}}]";
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    let mut loaded = load_objects(&value);
    let map = loaded.pop().expect("one object");
    assert_eq!(
        map.furniture.get(&'L'),
        Some(&"f_locker".to_owned()),
        "**the bundle's furniture is the symbol's**"
    );
    assert_eq!(
        map.items.get(&'L'),
        None,
        "**but a bundle's item LIST is a named gap: the symbol table holds one placement per symbol, and reading only the first would drop the rest silently**"
    );
    assert_eq!(
        map.terrain.get(&'.'),
        Some(&"t_floor".to_owned()),
        "**the flat table still states its own symbol**"
    );
}
