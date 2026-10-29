//! What the world emits by itself: terrain, furniture and the fields standing on them.
//!
//! Source: `map::generate_lightmap`'s gathering loop (`src/lightmap.cpp:525-547`), which adds a light source for any terrain, furniture or field whose own data says it
//! emits one. The port gathered only the `LightSource` components a caller had spawned, so a burning square lit nothing and a lamp terrain was dark.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::level::Level;
use cdda_sim::field_table::FieldTable;
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{Field, Map, Position, Prototypes, cast_lights_with, clear_light, set_field_table};
use cdda_sim::{light};

const FLOOR: u32 = 1;
const LAMP: u32 = 2;
const FIRE: u32 = 2;

fn world(marks: &[(i32, i32, u32)]) -> World {
    let mut world = World::new();
    let mut level = Level::new();
    level.ensure(0, 0, FLOOR);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, FLOOR, FLOOR);
        }
    }
    for (x, y, code) in marks {
        level.set_terrain(*x, *y, *code, FLOOR);
    }
    world.insert_resource(Map(level));

    let mut terrain = TerrainTable::new();
    terrain.insert(FLOOR, true);
    terrain.insert(LAMP, true);
    // A lamp terrain emits 120, which is what the data gives the lit carpet and the portal terrains (`terrain.json`).
    terrain.insert_light(LAMP, 120.0);
    cdda_sim::set_prototypes(&mut world, Prototypes {
        terrain,
        furniture: TerrainTable::new(),
    });

    let mut fields = FieldTable::new();
    // `fd_fire` emits 20, 60 and 160 at its three intensities, and smoke emits none (`fields.json`).
    fields.insert(FIRE, &[1.0, 10.0, 0.0]);
    fields.insert_light(FIRE, &[20.0, 60.0, 160.0]);
    set_field_table(&mut world, fields);
    world
}

#[test]
fn a_lamp_terrain_lights_its_own_square() {
    // `terrain->light_emitted > 0` (`lightmap.cpp:527-529`): the light is the terrain's own number, not something a caller placed.
    let mut world = world(&[(5, 5, LAMP)]);
    cast_lights_with(&mut world);
    assert!(
        light(&world).max_at(5, 5) > 50.0,
        "the lamp terrain is bright: {}",
        light(&world).max_at(5, 5)
    );
}

#[test]
fn a_plain_floor_emits_nothing() {
    let mut world = world(&[]);
    cast_lights_with(&mut world);
    assert_eq!(light(&world).max_at(5, 5), 0.0);
}

#[test]
fn a_fire_field_lights_the_room_by_its_own_intensity() {
    // `cur->get_intensity_level().light_emitted` (`lightmap.cpp:535-541`): a burning square is a light source, and its brightness is the field's own per-intensity number.
    let mut world = world(&[]);
    let mut entry = FieldEntry::new(3, 30, false, true);
    entry.set_intensity(3);
    world.spawn((
        Position { x: 5, y: 5, z: 0 },
        Field {
            id: FieldId(FIRE),
            entry,
            priority: 0,
        },
    ));
    cast_lights_with(&mut world);
    let at_the_fire = light(&world).max_at(5, 5);
    let across_the_room = light(&world).max_at(8, 5);

    clear_light(&mut world);
    let mut dim = world_spawn(1);
    cast_lights_with(&mut dim);
    let dim_at_the_fire = light(&dim).max_at(5, 5);

    assert!(
        at_the_fire > dim_at_the_fire,
        "the third intensity is the brightest: {at_the_fire} against {dim_at_the_fire}"
    );
    assert!(
        across_the_room > 0.0,
        "and it lights the squares around it: {across_the_room}"
    );
}

/// A world with a fire at the given intensity, built the same way by both halves of the test above.
fn world_spawn(intensity: i32) -> World {
    let mut world = world(&[]);
    let mut entry = FieldEntry::new(3, 30, false, true);
    entry.set_intensity(intensity);
    world.spawn((
        Position { x: 5, y: 5, z: 0 },
        Field {
            id: FieldId(FIRE),
            entry,
            priority: 0,
        },
    ));
    world
}

#[test]
fn two_lamps_beside_each_other_light_the_room_and_leave_no_buffer_behind() {
    // The reference records every emitter the map has, then applies them in one pass (`lightmap.cpp:463-464`, `654-659`). The skip rule must not darken anything: the dim lamp's
    // own square keeps its own light, the bright one's room keeps the bright value, and the buffer is empty once the pass has run, because the next build starts from zero.
    let mut world = world(&[(5, 5, LAMP)]);
    // A second, dimmer lamp beside it: the same terrain code with a different luminance, which the table cannot express, so the dimmer one is a fire that emits 20.
    let mut entry = FieldEntry::new(3, 30, false, true);
    entry.set_intensity(1);
    world.spawn((
        Position { x: 6, y: 5, z: 0 },
        Field {
            id: FieldId(FIRE),
            entry,
            priority: 0,
        },
    ));
    cast_lights_with(&mut world);

    let light = light(&world);
    assert!(
        light.max_at(5, 5) >= 100.0,
        "the lamp's own square: {}",
        light.max_at(5, 5)
    );
    assert!(
        light.max_at(6, 5) > 0.0,
        "the fire beside it is lit too: {}",
        light.max_at(6, 5)
    );
    assert!(
        light.max_at(9, 5) > 50.0,
        "and the room east of the pair keeps the lamp's light: {}",
        light.max_at(9, 5)
    );
    let map = world.resource::<Map>();
    assert_eq!(map.0.buffered_sources(), 0, "the buffer is drained");
}
