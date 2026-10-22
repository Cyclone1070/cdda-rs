//! What actually surrounds the square whose map chunk the port is trying to reproduce.
//!
//! The forest generator's arithmetic is driven by the four **cardinally adjacent overmap terrains**: their density sets
//! the ceiling the square is weighed against, and a neighbour that owns a forest biome contributes its own ground and
//! features rather than falling back to the region's default. So which neighbours a square has is not a detail - it is
//! most of the rule.
//!
//! The port assumed they were open field, and the reference's own world says otherwise.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::items_after_statements
)]

fn world() -> (Vec<Vec<String>>, (i32, i32)) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/overmap/reference-world.json");
    let cdda_json::Value::Object(root) =
        cdda_json::parse(&std::fs::read(&path).expect("the world")).expect("valid json")
    else {
        panic!("an object");
    };
    let Some(cdda_json::Value::Array(centre)) = root.get("centre") else {
        panic!("a centre");
    };
    let coordinate = |index: usize| match &centre[index] {
        cdda_json::Value::Int(number) => *number as i32,
        _ => panic!("a number"),
    };
    let Some(cdda_json::Value::Array(rows)) = root.get("rows") else {
        panic!("rows");
    };
    // Each row is one **string** of glyphs, one glyph per overmap square - the dump writes them that way so a row is
    // readable in the file.
    let rows = rows
        .iter()
        .map(|row| match row {
            cdda_json::Value::Str(text) => text.chars().map(String::from).collect(),
            _ => panic!("a row"),
        })
        .collect();
    (rows, (coordinate(0), coordinate(1)))
}

#[test]
fn the_chunk_s_square_is_surrounded_by_road_forest_and_field() {
    let (rows, centre) = world();
    // The world dump covers the centre plus and minus its radius, so the offset from the centre is the difference.
    const RADIUS: i32 = 20;
    let at = |x: i32, y: i32| -> String {
        rows[(y - (centre.1 - RADIUS)) as usize][(x - (centre.0 - RADIUS)) as usize].clone()
    };
    assert_eq!(at(centre.0, centre.1), "F", "the square itself is forest");
    assert_eq!(
        at(centre.0, centre.1 - 1),
        "─",
        "a road runs along its north side"
    );
    assert_eq!(at(centre.0 + 1, centre.1), "F", "forest to the east");
    assert_eq!(at(centre.0 - 1, centre.1), "F", "forest to the west");
    assert_eq!(
        at(centre.0, centre.1 + 1),
        ".",
        "and open ground to the south"
    );
}

#[test]
fn the_port_s_assumption_of_open_field_on_all_four_sides_is_wrong() {
    // This is the record of a wrong assumption rather than a rule. The generator's pass was written and measured with
    // all four neighbours treated as open field - no biome, no density, no palette - and this square has forest on two
    // sides and a road on a third. Every one of those neighbours changes the arithmetic: the forests give the square a
    // density ceiling and their own ground palettes, and the road may be given a biome in the region's settings
    // precisely so that the woodland does not fade out where it meets it.
    let (rows, centre) = world();
    const RADIUS: i32 = 20;
    let at = |x: i32, y: i32| -> String {
        rows[(y - (centre.1 - RADIUS)) as usize][(x - (centre.0 - RADIUS)) as usize].clone()
    };
    let neighbours = [
        at(centre.0, centre.1 - 1),
        at(centre.0 + 1, centre.1),
        at(centre.0, centre.1 + 1),
        at(centre.0 - 1, centre.1),
    ];
    let forest_neighbours = neighbours.iter().filter(|glyph| *glyph == "F").count();
    assert_eq!(forest_neighbours, 2, "two of the four: {neighbours:?}");
    assert!(
        neighbours.iter().any(|glyph| *glyph == "─"),
        "and a road: {neighbours:?}"
    );
}
