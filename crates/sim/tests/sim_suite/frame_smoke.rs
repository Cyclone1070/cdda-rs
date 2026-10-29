//! Smoke in front of a square, and what the frame does about it.
//!
//! The reference's sight cast reads `transparency_cache`, which is a square's terrain **and** furniture **and** fields multiplied together (`calc_transp`,
//! `lightmap.cpp:140-167`), so smoke between a character and a square attenuates the view of that square exactly as a wall would end it. The frame built its
//! transparency table from the terrain alone, which made every field invisible to the sight cast: a square behind smoke was drawn as plainly as an open one.

#![allow(clippy::panic, clippy::expect_used, clippy::float_cmp)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::level::Level;
use cdda_sim::field_table::FieldTable;
use cdda_sim::frame::frame;
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{Field, Map, Position, Prototypes, cast_lights_with, set_field_table};

const FLOOR: u32 = 1;
const SMOKE: u32 = 7;

fn world(smoke: i32) -> World {
    let mut world = World::new();
    let mut level = Level::new();
    for y in 0..20 {
        for x in 0..20 {
            level.set_terrain(x, y, FLOOR, FLOOR);
        }
    }
    world.insert_resource(Map(level));

    let mut terrain = TerrainTable::new();
    terrain.insert(FLOOR, true);
    terrain.insert_symbol(FLOOR, '.');
    cdda_sim::set_prototypes(&mut world, Prototypes {
        terrain,
        furniture: TerrainTable::new(),
    });

    // **Smoke's own numbers**: translucent at its first intensity and all but solid at its third (`src/field.cpp`), which is why a room fills up with it.
    let mut table = FieldTable::new();
    table.insert(SMOKE, &[3.0, 10.0, 0.0]);
    set_field_table(&mut world, table);

    if smoke > 0 {
        let mut entry = FieldEntry::new(3, 100, false, false);
        entry.set_intensity(smoke);
        world.spawn((
            Position { x: 7, y: 5, z: 0 },
            Field {
                id: FieldId(SMOKE),
                entry,
                priority: 0,
            },
        ));
    }
    world
}

#[test]
fn smoke_between_the_character_and_a_square_thins_the_view_of_it() {
    let mut clear = world(0);
    cast_lights_with(&mut clear);
    let seen_clear = frame(&mut clear, (5, 5), 6, 10.0, 3.0)
        .at(9, 5)
        .expect("a floor")
        .seen;
    assert!(seen_clear > 0.0, "an open square four away is seen");

    let mut smoky = world(2);
    cast_lights_with(&mut smoky);
    let seen_smoke = frame(&mut smoky, (5, 5), 6, 10.0, 3.0)
        .at(9, 5)
        .expect("a floor")
        .seen;
    assert!(
        seen_smoke < seen_clear,
        "the same square behind smoke is seen less: {seen_smoke} against {seen_clear}"
    );
}

#[test]
fn dense_smoke_hides_what_is_behind_it_entirely() {
    let mut smoky = world(3);
    cast_lights_with(&mut smoky);
    let drawn = frame(&mut smoky, (5, 5), 6, 10.0, 3.0);
    let seen = drawn.at(9, 5).expect("a floor").seen;
    assert_eq!(seen, 0.0, "smoke at its third intensity is solid to a sight line");
}
