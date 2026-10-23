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
use cdda_core_types::shadowcasting::{FourQuadrants, Quadrant, sight_calc};
use cdda_core_types::transparency::{FieldOpacity, SquareFacts};
use std::collections::BTreeMap;

/// A square of open air, with whatever fields the test gives it.
fn air(fields: Vec<FieldOpacity>) -> SquareFacts {
    SquareFacts {
        terrain_transparent: true,
        furniture_transparent: true,
        outside: false,
        fields,
    }
}

fn smoke(translucency: f32) -> FieldOpacity {
    FieldOpacity {
        transparent: false,
        translucency,
    }
}

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

/// Terrain id used by these tests to mean "a wall".
const WALL: u32 = 2;

fn light_from(
    level: &mut Level,
    origin: (i32, i32),
    luminance: f32,
) -> BTreeMap<(i32, i32), FourQuadrants> {
    // The transparency rule is asked about squares by the caster while the level is borrowed mutably, so the level's terrain is snapshotted first.
    let mut terrains = BTreeMap::new();
    for y in 0..12 {
        for x in 0..12 {
            terrains.insert((x, y), level.terrain_at(x, y).unwrap_or(0));
        }
    }
    let mut light = BTreeMap::new();
    level.add_light_source_with(
        origin,
        luminance,
        1.0,
        move |x, y| SquareFacts {
            terrain_transparent: terrains.get(&(x, y)).copied().unwrap_or(0) != WALL,
            furniture_transparent: true,
            outside: false,
            fields: Vec::new(),
        },
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
fn a_wall_lights_the_quarter_that_faces_the_lamp_and_a_floor_lights_only_the_default_one() {
    // The reference's own rule, and it is not what it looks like: `castLight` writes the octant's quadrant for an **opaque** square and
    // `quadrant::default_` for a transparent one (`lightmap.cpp:943-951`), with a comment saying why - "for non-opaque tiles direction doesn't matter so we
    // just use the single default_ value". `default_` is `NE` (`shadowcasting.h:26-31`), so a floor tile gets exactly one quarter and a wall gets the two facing
    // the lamp.
    let mut level = level();
    // A wall at (5, 4), north-west of the lamp at (6, 5).
    level.set_terrain(5, 4, WALL, FLOOR);
    let light = light_from(&mut level, (6, 5), 10.0);

    let wall = light.get(&(5, 4)).expect("the wall is lit");
    assert!(
        wall.get(Quadrant::SouthEast) > 0.0,
        "the quarter facing the lamp"
    );
    assert_eq!(
        wall.get(Quadrant::NorthWest),
        0.0,
        "and not the one facing away, which is what makes a wall read as a wall"
    );

    let floor = light
        .get(&(6, 4))
        .expect("the floor north of the lamp is lit");
    assert!(floor.get(Quadrant::NorthEast) > 0.0, "the default quarter");
    assert_eq!(
        floor.get(Quadrant::SouthWest),
        0.0,
        "and only the default quarter, because a floor has no side to face"
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
    let clear = |_x: i32, _y: i32| air(Vec::new());
    level.add_light_source_with((5, 5), 10.0, 1.0, clear, &mut light);
    let first = light[&(6, 5)].max();
    level.add_light_source_with((6, 5), 10.0, 1.0, clear, &mut light);
    let second = light[&(6, 5)].max();
    assert!(second >= first, "adding a lamp does not dim the tile");
    assert!(second < first * 2.0, "and does not double it either");
}

#[test]
fn fields_darken_a_ray_in_proportion_rather_than_by_the_worst_one() {
    // The rule the level used to get wrong: `calc_transp` **multiplies** each field's translucency (`lightmap.cpp:158-165`), so two layers of half-opaque smoke leave a quarter
    // of the light, not a half. The old closure API took one number per square and kept the most opaque field, which is a different rule and a wrong one.
    let mut level = level();
    let smoker = |x: i32, y: i32| -> SquareFacts {
        if (4..=6).contains(&x) && (4..=6).contains(&y) {
            air(vec![smoke(0.5), smoke(0.5)])
        } else {
            air(Vec::new())
        }
    };
    let mut light = BTreeMap::new();
    level.add_light_source_with((1, 5), 10.0, 1.0, smoker, &mut light);

    let expected = sight_calc(
        10.0,
        cdda_core_types::light::TRANSPARENCY_OPEN_AIR * 0.25,
        6,
    );
    let through = light
        .get(&(7, 5))
        .copied()
        .expect("the far side is lit")
        .max();
    assert!(
        (through - expected).abs() < 2.0,
        "two layers of smoke, one quarter of the light: got {through}, expected about {expected}"
    );
}
