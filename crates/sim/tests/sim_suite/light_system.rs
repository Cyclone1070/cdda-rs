//! Light as a system over the store: it reads what is where and writes an answer.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::level::Level;
use cdda_sim::field_table::FieldTable;
use cdda_sim::set_field_table;
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{
    light,
    Field, LightSource, Map, Position, Prototypes, SOLID_TERRAIN, cast_lights_with,
    clear_light,
};

/// The prototypes a world with only floors and walls would have loaded: floor code 1 is air, code 0 is solid.
fn prototypes() -> Prototypes {
    let mut terrain = TerrainTable::new();
    terrain.insert(1, true);
    terrain.insert(SOLID_TERRAIN, false);
    let mut furniture = TerrainTable::new();
    furniture.insert(0, true);
    Prototypes { terrain, furniture }
}

/// The field table the data gives: smoke's three levels, in the loaded numbers.
fn field_table() -> FieldTable {
    let mut fields = FieldTable::new();
    fields.insert(1, &[1.0, 10.0, 0.0]);
    fields
}

fn world_with_floor() -> World {
    let mut world = World::new();
    let mut level = Level::new();
    level.ensure(0, 0, 1);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, 1, 1);
        }
    }
    cdda_sim::load_level(&mut world, level);
    cdda_sim::set_prototypes(&mut world, prototypes());
    set_field_table(&mut world, field_table());
    world
}

#[test]
fn a_torch_lights_the_squares_around_it() {
    let mut world = world_with_floor();
    world.spawn((
        Position { x: 5, y: 5, z: 0 },
        LightSource { luminance: 10.0 },
    ));
    cast_lights_with(&mut world);

    let light = light(&world);
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
    cast_lights_with(&mut world);
    let clear = light(&world).max_at(6, 5);

    // Intensity two: the data gives smoke a translucency of ten there, which dims without stopping. Intensity three is zero and would make the square solid, which is
    // the next test's business rather than this one's.
    let mut entry = FieldEntry::new(3, 100, true, false);
    entry.set_intensity(2);
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
    //
    // Ten is what the data gives smoke at its second intensity (`field_type.json`: `translucency: 10`), and the value is a *multiplier on
    // the square's light transparency*, where higher means more absorbing. An intuitive 0.38 would make the smoke more transparent than open
    // air and brighten everything behind it - which is exactly what this test caught when the cast was first joined to the transparency rule.
    clear_light(&mut world);
    cast_lights_with(&mut world);

    let smoked = light(&world).max_at(6, 5);
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
    cast_lights_with(&mut world);

    let light = light(&world);
    assert!(light.max_at(7, 5) > 0.0, "in front of the wall");
    assert_eq!(light.max_at(10, 5), 0.0, "behind it");
}

#[test]
fn with_nothing_shining_the_answer_is_empty() {
    let mut world = world_with_floor();
    cast_lights_with(&mut world);
    assert_eq!(light(&world).lit_squares(), 0);
}
