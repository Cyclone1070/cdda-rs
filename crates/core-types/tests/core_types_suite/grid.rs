//! The sizes of the world: a submap, the reality bubble, and the vertical stack.
//!
//! Two dumps are the evidence, and they disagree in a way worth keeping:
//!
//! * `corpus/geometry.json` (from `--dump-geometry`) says the **map** is 132 by 132 squares - `MAPSIZE_X` by `MAPSIZE_Y`, eleven submaps of twelve - and that
//!   the vertical stack is 21 levels, from ten below zero to ten above;
//! * `corpus/state/replay-walk-east.json` says the **bubble** the state dump fingerprints is 14,641 tiles, which is 121 by 121 - `2 * MAX_VIEW_DISTANCE + 1`.
//!
//! They are different things: the map is what is loaded, the bubble is what was looked at. A port that conflates them is off by eleven squares in every
//! direction and would not notice until something read the edge of the bubble.

#![allow(clippy::expect_used, clippy::panic)]

use cdda_core_types::grid;

fn geometry() -> cdda_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/geometry.json");
    let text = std::fs::read_to_string(&path).expect("the geometry dump");
    cdda_json::parse(text.as_bytes()).expect("it parses")
}

/// A count as the dump states it. The sizes here are all far below any integer limit, so the widening cannot lose anything.
#[allow(clippy::cast_possible_wrap)]
fn count(value: usize) -> i64 {
    value as i64
}

fn integer(value: &cdda_json::Value, key: &str) -> i64 {
    match value {
        cdda_json::Value::Object(object) => match object.get(key) {
            Some(cdda_json::Value::Int(number)) => *number,
            other => panic!("{key} is not an integer: {other:?}"),
        },
        other => panic!("the dump is not an object: {other:?}"),
    }
}

#[test]
fn every_size_is_the_references_own_number() {
    let dump = geometry();
    assert_eq!(i64::from(grid::SEEX), integer(&dump, "seex"), "SEEX");
    assert_eq!(i64::from(grid::SEEY), integer(&dump, "seey"), "SEEY");
    assert_eq!(
        i64::from(grid::MAPSIZE),
        integer(&dump, "mapsize"),
        "MAPSIZE"
    );
    assert_eq!(
        i64::from(grid::HALF_MAPSIZE),
        integer(&dump, "half_mapsize"),
        "HALF_MAPSIZE"
    );
    assert_eq!(
        i64::from(grid::MAPSIZE_X),
        integer(&dump, "mapsize_x"),
        "MAPSIZE_X"
    );
    assert_eq!(
        i64::from(grid::OVERMAP_DEPTH),
        integer(&dump, "overmap_depth"),
        "OVERMAP_DEPTH"
    );
    assert_eq!(
        i64::from(grid::OVERMAP_HEIGHT),
        integer(&dump, "overmap_height"),
        "OVERMAP_HEIGHT"
    );
    assert_eq!(
        i64::from(grid::OVERMAP_LAYERS),
        integer(&dump, "overmap_layers"),
        "OVERMAP_LAYERS"
    );
    assert_eq!(
        i64::from(grid::MAX_VIEW_DISTANCE),
        integer(&dump, "max_view_distance"),
        "MAX_VIEW_DISTANCE"
    );
    assert_eq!(
        count(grid::SUBMAP_ELEMENTS),
        integer(&dump, "submap_elements"),
        "a submap's squares"
    );
    assert_eq!(
        count(grid::MAP_TILES),
        integer(&dump, "bubble_tiles"),
        "the map's squares"
    );
}

#[test]
fn the_view_is_121_squares_across_and_the_map_is_132() {
    // The state dump the replay gate compares is a 121 by 121 window; the map it was taken from is 132 by 132. Both numbers are in the corpus, and they are not
    // the same number.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/state/replay-walk-east.json");
    let text = std::fs::read_to_string(&path).expect("the state dump");
    let state = cdda_json::parse(text.as_bytes()).expect("it parses");
    let cdda_json::Value::Object(root) = &state else {
        panic!("the state dump is an object");
    };
    let Some(cdda_json::Value::Object(bubble)) = root.get("bubble") else {
        panic!("it holds a bubble");
    };
    assert_eq!(
        integer(&cdda_json::Value::Object(bubble.clone()), "tiles"),
        14_641,
        "the state dump's own count"
    );
    assert_eq!(
        count(grid::VIEW_TILES),
        14_641,
        "2 * MAX_VIEW_DISTANCE + 1, squared"
    );
    assert_ne!(
        grid::VIEW_TILES,
        grid::MAP_TILES,
        "the two are different things"
    );
}

#[test]
fn a_submaps_squares_are_indexed_by_x_then_y() {
    // Transcribed: `maptile_soa` stores `ter[x][y]` (`submap.h:86-92`), so the flat index is `x * SEEY + y` and **not** the row-major `x + y * SEEX`. A port
    // that picks the other one transposes every submap, which looks fine until something walks a wall.
    assert_eq!(grid::submap_index(0, 0), 0);
    assert_eq!(grid::submap_index(1, 0), 12, "the next x is a whole row");
    assert_eq!(grid::submap_index(0, 1), 1, "the next y is one square");
    assert_eq!(grid::submap_index(11, 11), 143, "the last square");
}

#[test]
fn a_submap_is_found_by_x_then_y_but_a_layer_comes_first() {
    // Transcribed: `map::get_nonant` (`map.cpp:9715-9732`). With z-levels on the index is
    // `(z + OVERMAP_DEPTH) + (x + y * MAPSIZE) * OVERMAP_LAYERS`, so **z is the outer dimension** and the whole stack of one submap column is contiguous.
    // Without z-levels it is the plain `x + y * MAPSIZE`.
    assert_eq!(grid::nonant(0, 0, 0, true), 10, "z = 0 sits ten layers in");
    assert_eq!(
        grid::nonant(0, 0, -10, true),
        0,
        "the lowest level is first"
    );
    assert_eq!(
        grid::nonant(0, 0, 10, true),
        20,
        "the highest level is last in its column"
    );
    assert_eq!(
        grid::nonant(1, 0, 0, true),
        10 + 21,
        "the next x moves a whole layer's worth"
    );
    assert_eq!(
        grid::nonant(0, 0, 0, false),
        0,
        "without z-levels there is one layer"
    );
    assert_eq!(grid::nonant(0, 1, 0, false), 11, "and y is the row");
}
