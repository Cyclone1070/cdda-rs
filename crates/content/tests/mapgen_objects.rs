//! Mapgen objects: the rows a JSON map states and what each glyph means.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::mapgen::{MapgenObject, load_objects};

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
    let text = format!(
        "[{{\"method\":\"json\",\"om_terrain\":\"test\",\"rows\":[{rows}],\"terrain\":{{{terrain}}}}}]"
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

// The corpus gate for this loader is deliberately absent: walking the reference's mapgen
// directory through `json_files_in_load_order` + `read_json` returned zero objects, while the files
// themselves are plainly arrays of mapgen objects. Until that read path is understood, a green gate
// here would be a gate over nothing - so the loader is tested against hand-written objects and the
// corpus gate waits for the next round rather than passing vacuously.
