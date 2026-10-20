//! A light source on the level: what it lights, how brightly, and which corner.
//!
//! Source: `map::apply_light_source` (`src/lightmap.cpp`). These are property tests, not a
//! parity gate, and the ledger says so: the reference accumulates transparency along each ray
//! and this uses open air, so the numbers are not claimed to match its output.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

use cdda_core_types::level::Level;
use cdda_core_types::shadowcasting::{FourQuadrants, Quadrant};
use std::collections::BTreeMap;

const FLOOR: u32 = 1;

fn level() -> Level {
    let mut level = Level::new();
    level.ensure(0, 0, FLOOR);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, FLOOR, FLOOR);
        }
    }
    level
}

fn light_from(
    level: &mut Level,
    origin: (i32, i32),
    luminance: f32,
) -> BTreeMap<(i32, i32), FourQuadrants> {
    let mut light = BTreeMap::new();
    level.add_light_source(
        origin,
        luminance,
        |terrain| if terrain == 2 { 0.0 } else { 2.3 / 72.0 },
        |_x, _y| 0.0,
        &mut light,
    );
    light
}

#[test]
fn a_lamp_lights_the_room_around_it_and_not_the_whole_map() {
    let mut level = level();
    let light = light_from(&mut level, (5, 5), 10.0);
    // The level is only twelve tiles across and any source bright enough to cast at all
    // reaches further than that, so the claim worth testing is the falloff, not the edge.
    assert!(light.contains_key(&(6, 5)), "the tile next door is lit");
    let near = light[&(6, 5)].max();
    let far = light[&(9, 9)].max();
    assert!(near > far, "closer is brighter: {near} then {far}");
}

#[test]
fn the_corner_facing_the_light_is_the_bright_one() {
    // The lamp is south-east of the tile at (5,4), so that tile's south-east quarter takes the
    // light and its north-west quarter does not.
    let mut level = level();
    let light = light_from(&mut level, (6, 5), 10.0);
    let tile = light.get(&(5, 4)).expect("the tile is lit");
    assert!(
        tile.get(Quadrant::SouthEast) > 0.0,
        "the corner facing the lamp"
    );
    assert_eq!(
        tile.get(Quadrant::NorthWest),
        0.0,
        "the corner away from it"
    );
}

#[test]
fn a_wall_stops_the_light_it_casts_shadows_on() {
    let mut level = level();
    for y in 0..12 {
        level.set_terrain(8, y, 2, FLOOR);
    }
    let light = light_from(&mut level, (5, 5), 10.0);
    assert!(light.contains_key(&(7, 5)), "in front of the wall");
    assert!(!light.contains_key(&(9, 5)), "behind it");
}

#[test]
fn a_dim_source_lights_nothing_at_all() {
    // Below LIGHT_AMBIENT_LOW there is no range to cast into.
    let mut level = level();
    let light = light_from(&mut level, (5, 5), 1.0);
    assert!(light.is_empty(), "a candle stub is not a lamp");
}

#[test]
fn two_sources_take_the_brighter_corner_rather_than_summing() {
    let mut level = level();
    let mut light = BTreeMap::new();
    level.add_light_source(
        (5, 5),
        10.0,
        |terrain| if terrain == 2 { 0.0 } else { 2.3 / 72.0 },
        |_x, _y| 0.0,
        &mut light,
    );
    let first = light[&(6, 5)].max();
    level.add_light_source(
        (6, 5),
        10.0,
        |terrain| if terrain == 2 { 0.0 } else { 2.3 / 72.0 },
        |_x, _y| 0.0,
        &mut light,
    );
    let second = light[&(6, 5)].max();
    assert!(second >= first, "adding a lamp does not dim the tile");
    assert!(second < first * 2.0, "and does not double it either");
}
