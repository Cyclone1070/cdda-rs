//! Writing a map into a level, glyph by glyph.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_content::mapgen::load_objects;
use cdda_core_types::level::Level;

const FLOOR: u32 = 1;
const WALL: u32 = 2;
const BLANK: u32 = 9;

fn map(text: &str) -> cdda_content::mapgen::MapgenObject {
    let value = cdda_json::parse(text.as_bytes()).expect("valid json");
    load_objects(&value).pop().expect("one map")
}

fn dressed() -> String {
    // The real shape: rows and symbol tables under `object`.
    r##"[{"type":"mapgen","method":"json","om_terrain":"test","object":{
        "rows":["..#","..."],
        "terrain":{".":"t_floor","#":"t_wall"},
        "furniture":{"f":"f_table"}}}]"##
        .to_owned()
}

fn ids(id: &str) -> Option<u32> {
    match id {
        "t_floor" => Some(FLOOR),
        "t_wall" => Some(WALL),
        _ => None,
    }
}

#[test]
fn a_map_is_written_glyph_by_glyph() {
    let mut level = Level::new();
    let placed = map(&dressed()).place(&mut level, (0, 0), BLANK, ids);
    assert_eq!(placed.squares, 6);
    assert_eq!(level.terrain_at(0, 0), Some(FLOOR));
    assert_eq!(level.terrain_at(2, 0), Some(WALL));
    assert_eq!(
        level.terrain_at(2, 1),
        Some(FLOOR),
        "the second row was written too"
    );
    assert!(placed.unknown_symbols.is_empty());
}

#[test]
fn the_origin_moves_the_whole_map() {
    let mut level = Level::new();
    map(&dressed()).place(&mut level, (10, 5), BLANK, ids);
    assert_eq!(
        level.terrain_at(10, 5),
        Some(FLOOR),
        "top left at the origin"
    );
    assert_eq!(level.terrain_at(12, 5), Some(WALL));
    assert_ne!(
        level.terrain_at(0, 0),
        Some(FLOOR),
        "the old origin is untouched"
    );
}

#[test]
fn a_symbol_the_map_does_not_explain_is_reported_rather_than_becoming_floor() {
    let text = r#"[{"type":"mapgen","method":"json","om_terrain":"test","object":{
        "rows":["?."],"terrain":{".":"t_floor"}}}]"#;
    let mut level = Level::new();
    let placed = map(text).place(&mut level, (0, 0), BLANK, ids);
    assert_eq!(placed.unknown_symbols, vec!['?']);
    assert_eq!(
        level.terrain_at(0, 0),
        Some(BLANK),
        "left blank, not guessed"
    );
}

#[test]
fn furniture_is_reported_and_its_square_is_floor_underneath() {
    let text = r#"[{"type":"mapgen","method":"json","om_terrain":"test","object":{
        "rows":["f"],"terrain":{".":"t_floor"},"furniture":{"f":"f_table"}}}]"#;
    let mut level = Level::new();
    let placed = map(text).place(&mut level, (0, 0), FLOOR, ids);
    assert_eq!(placed.furniture, vec![(0, 0, "f_table".to_owned())]);
    assert!(
        placed.unknown_symbols.is_empty(),
        "a furniture symbol is not unknown"
    );
    assert_eq!(level.terrain_at(0, 0), Some(FLOOR), "floor under the table");
}

#[test]
fn an_id_the_content_does_not_know_is_reported() {
    let text = r#"[{"type":"mapgen","method":"json","om_terrain":"test","object":{
        "rows":["x"],"terrain":{"x":"t_nonexistent"}}}]"#;
    let mut level = Level::new();
    let placed = map(text).place(&mut level, (0, 0), BLANK, ids);
    assert_eq!(placed.unknown_symbols, vec!['x']);
    assert_eq!(level.terrain_at(0, 0), Some(BLANK));
}
