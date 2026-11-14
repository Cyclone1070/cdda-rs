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

#[test]
fn the_map_s_own_light_at_is_four_arms_and_never_says_bright_only() {
    // **`map::light_at` (`src/lightmap.cpp:674-697`), transcribed.** It is not `apparent_light_at`: it knows nothing about obstruction, range or remembering, and it is
    // what the draw path asks first. The four arms are, in the reference's order, a source in the square (BRIGHT), then `LIGHT_AMBIENT_LIT` (LIT), then
    // `LIGHT_AMBIENT_LOW` (LOW), then DARK - so a square with a source of ten is BRIGHT even when it is otherwise black, and a square at 9.999 is not.
    use cdda_core_types::light::{LitLevel, light_at};

    assert_eq!(
        light_at(10.0, 0.0),
        LitLevel::Bright,
        "a source at the threshold is bright"
    );
    assert_eq!(
        light_at(9.99, 9.99),
        LitLevel::Low,
        "just under it is not, whatever the field says"
    );
    assert_eq!(
        light_at(0.0, 10.0),
        LitLevel::Lit,
        "the lit threshold is inclusive"
    );
    assert_eq!(
        light_at(0.0, 9.99),
        LitLevel::Low,
        "and just under it is low"
    );
    assert_eq!(
        light_at(0.0, 3.5),
        LitLevel::Low,
        "the low threshold is inclusive too"
    );
    assert_eq!(
        light_at(0.0, 3.49),
        LitLevel::Dark,
        "below both thresholds there is nothing to see by"
    );
    // **Never `BRIGHT_ONLY`.** That value belongs to `apparent_light_at`, which also weighs obstruction; a `light_at` that returned it would make every caller that
    // matches on four arms wrong.
    for source in [0.0, 5.0, 10.0, 1e6] {
        for light in [0.0, 3.5, 10.0, 1e6] {
            assert_ne!(light_at(source, light), LitLevel::BrightOnly);
        }
    }
}
