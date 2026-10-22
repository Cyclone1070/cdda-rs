//! Light as a system over the store: it reads what is where and writes an answer.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::level::Level;
use cdda_sim::{Field, Light, LightSource, Map, Position, SOLID_TERRAIN, cast_lights, clear_light};

fn world_with_floor() -> World {
    let mut world = World::new();
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, 1, 1);
        }
    }
    world.insert_resource(Map(level));
    world
}

#[test]
fn a_torch_lights_the_squares_around_it() {
    let mut world = world_with_floor();
    world.spawn((
        Position { x: 5, y: 5, z: 0 },
        LightSource { luminance: 10.0 },
    ));
    cast_lights(&mut world, Map::terrain_transparency, |_id, _intensity| 0.0);

    let light = world.resource::<Light>();
    assert!(light.lit_squares() > 0, "something is lit");
    assert!(light.max_at(6, 5) > 0.0, "the square next door");
    assert!(
        light.max_at(6, 5) > light.max_at(9, 9),
        "and nearer is brighter"
    );
}

#[test]
fn smoke_standing_on_a_square_dims_the_light_that_crosses_it() {
    // The whole point of the store: the smoke is an entity, and the lighting system sees it
    // because it asks what is standing on each square - not because the map keeps a copy.
    let mut world = world_with_floor();
    world.spawn((
        Position { x: 3, y: 5, z: 0 },
        LightSource { luminance: 10.0 },
    ));
    cast_lights(&mut world, Map::terrain_transparency, |_id, _intensity| 0.0);
    let clear = world.resource::<Light>().max_at(6, 5);

    let mut entry = FieldEntry::new(3, 100, true, false);
    entry.set_intensity(3);
    world.spawn((
        Position { x: 5, y: 5, z: 0 },
        Field {
            id: FieldId(1),
            entry,
            priority: 0,
        },
    ));
    // A turn clears the answer before recomputing it: a stale brighter value would survive a
    // merge by maximum and hide the dimming entirely, which is what this test caught.
    clear_light(&mut world);
    cast_lights(&mut world, Map::terrain_transparency, |_id, _intensity| {
        0.38
    });

    let smoked = world.resource::<Light>().max_at(6, 5);
    assert!(
        smoked < clear && smoked > 0.0,
        "dimmer, not dark: {smoked} against {clear}"
    );
}

#[test]
fn a_wall_in_the_store_stops_the_light_it_shadows() {
    let mut world = world_with_floor();
    if let Some(mut map) = world.get_resource_mut::<Map>() {
        for y in 0..12 {
            map.0.set_terrain(8, y, SOLID_TERRAIN, 1);
        }
    }
    world.spawn((
        Position { x: 5, y: 5, z: 0 },
        LightSource { luminance: 10.0 },
    ));
    cast_lights(&mut world, Map::terrain_transparency, |_id, _intensity| 0.0);

    let light = world.resource::<Light>();
    assert!(light.max_at(7, 5) > 0.0, "in front of the wall");
    assert_eq!(light.max_at(10, 5), 0.0, "behind it");
}

#[test]
fn with_nothing_shining_the_answer_is_empty() {
    let mut world = world_with_floor();
    cast_lights(&mut world, Map::terrain_transparency, |_id, _intensity| 0.0);
    assert_eq!(world.resource::<Light>().lit_squares(), 0);
}
