//! The light thresholds, against values the reference printed.
//!
//! `light_range` is `src/lightmap.h:42-49` with the reference's own constants: `MAX_VIEW_DISTANCE` is `SEEX * HALF_MAPSIZE` = **60** (`map_scale_constants.h:12-27`), so
//! air loses `2.3/60` per square. The table below is that formula's output.
//!
//! It used to say 72 - the port's constant, not the reference's - and the whole table was the port agreeing with itself. The vision fixture caught the difference as
//! "one distance step brighter" along every wall, which is what a four-percent-too-transparent air looks like.

#![allow(clippy::panic)]

use cdda_core_types::light::{LitLevel, light_range, lit_level};

/// What the reference's `LIGHT_RANGE` returns for a brightness.
const RANGE_TABLE: [(f32, i32); 13] = [
    (0.0, 0),
    (0.1, 0),
    (1.0, 0),
    (3.4, 0),
    (3.5, 0),
    (3.6, 0),
    (5.0, 9),
    (9.999, 27),
    (10.0, 27),
    (10.001, 27),
    (20.0, 45),
    (60.0, 74),
    (100.0, 87),
];

#[test]
fn light_range_matches_the_reference_table() {
    for (brightness, expected) in RANGE_TABLE {
        assert_eq!(
            light_range(brightness),
            expected,
            "light_range({brightness}) should be {expected}"
        );
    }
}

#[test]
fn a_tiles_level_follows_the_reference_order() {
    // src/lightmap.cpp:674-696, top to bottom: out of bounds, then a bright source in
    // the tile, then full daylight, then anything worth seeing by, then dark.
    assert_eq!(lit_level(false, 1_000.0, 1_000.0), LitLevel::Dark);
    assert_eq!(lit_level(true, 10.0, 0.0), LitLevel::Bright);
    assert_eq!(lit_level(true, 9.999, 10.0), LitLevel::Lit);
    assert_eq!(lit_level(true, 0.0, 9.999), LitLevel::Low);
    assert_eq!(lit_level(true, 0.0, 3.5), LitLevel::Low);
    assert_eq!(lit_level(true, 0.0, 3.499), LitLevel::Dark);
    assert_eq!(lit_level(true, 0.0, 0.0), LitLevel::Dark);
}

#[test]
fn a_bright_source_outranks_full_daylight() {
    // A lamp in a sunlit room is bright, not merely lit: the source check comes first
    // in the reference, and swapping the two would be invisible until it mattered.
    assert_eq!(lit_level(true, 10.0, 40.0), LitLevel::Bright);
}
