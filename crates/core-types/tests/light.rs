//! The light thresholds, against values the reference printed.
//!
//! `light_range` was checked against a program built from `src/lightmap.h:42-49` with
//! the reference's own constants (`MAX_VIEW_DISTANCE` 72, air losing `2.3/72` per
//! square). The table below is that program's output, not my arithmetic.

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
    (5.0, 11),
    (9.999, 32),
    (10.0, 32),
    (10.001, 32),
    (20.0, 54),
    (60.0, 88),
    (100.0, 104),
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
