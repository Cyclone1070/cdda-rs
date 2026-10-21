//! Daylight on a level: the sun outside, and what reaches in through a door.
//!
//! Source: the ambient pass in `map::build_map_cache` (`src/lightmap.cpp:396-438`) plus the
//! sun's curve and the thresholds already ported.

#![allow(clippy::panic, clippy::float_cmp, clippy::expect_used)]

use cdda_core_types::level::Level;
use cdda_core_types::shadowcasting::Quadrant;

/// A level of one 12x12 submap where the given tiles are under open sky.
fn level(outside: &dyn Fn(i32, i32) -> bool) -> Level {
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    // Touch every tile so the whole submap has terrain to light.
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, 1, 1);
        }
    }
    let _ = outside;
    level
}

#[test]
fn an_outdoor_tile_takes_the_sun_in_all_four_quarters() {
    let level = level(&|_, _| false);
    let lit = level.daylight(12.0 * 3600.0, 0.0, |_, _| true);
    let tile = lit.get(&(5, 5)).expect("the tile exists");
    assert!(tile.max() > 60.0, "noon is bright: {}", tile.max());
    assert_eq!(
        tile.get(Quadrant::NorthEast),
        tile.get(Quadrant::SouthWest),
        "open sky lights the tile evenly"
    );
}

#[test]
fn an_under_roof_tile_with_open_sky_to_its_south_lights_only_the_south_quarters() {
    // One tile of open sky at (5,6); the tile north of it is under a roof.
    let level = level(&|_, _| false);
    let lit = level.daylight(12.0 * 3600.0, 0.0, |x, y| (x, y) == (5, 6));
    let indoors = lit.get(&(5, 5)).expect("the tile exists");
    assert!(
        indoors.get(Quadrant::SouthEast) > 60.0,
        "daylight comes in from the south"
    );
    assert!(indoors.get(Quadrant::SouthWest) > 60.0);
    assert_eq!(
        indoors.get(Quadrant::NorthEast),
        0.0,
        "the back of the room stays dark"
    );
    assert_eq!(indoors.get(Quadrant::NorthWest), 0.0);
}

#[test]
fn a_tile_far_from_any_opening_stays_at_the_indoor_level() {
    let level = level(&|_, _| false);
    let lit = level.daylight(12.0 * 3600.0, 5.0, |x, y| (x, y) == (5, 6));
    let far = lit.get(&(0, 0)).expect("the tile exists");
    assert_eq!(far.max(), 5.0, "no opening, no daylight");
}

#[test]
fn the_same_room_is_dark_at_night() {
    let level = level(&|_, _| false);
    let lit = level.daylight(0.0, 0.0, |x, y| (x, y) == (5, 6));
    let indoors = lit.get(&(5, 5)).expect("the tile exists");
    assert_eq!(indoors.max(), 0.0, "midnight lights nothing");
}
