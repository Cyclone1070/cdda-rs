//! What one square can see of another, with the world's own layers behind the answer.
//!
//! Source: `map::sees` (`src/map.cpp:7329-7362`) and `map::is_transparent`/`is_transparent_wo_fields`
//! (`src/map.cpp`), which read the transparency cache with the fields counted, or without them.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::level::Level;
use cdda_sim::field_table::FieldTable;
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{Field, Position, Prototypes, sees, set_field_table};

const FLOOR: u32 = 1;
const WALL: u32 = 2;
const SMOKE: u32 = 1;

fn world(terrain: &[(i32, i32, u32)]) -> World {
    let mut world = World::new();
    let mut level = Level::new();
    level.ensure(0, 0, FLOOR);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, FLOOR, FLOOR);
        }
    }
    for (x, y, code) in terrain {
        level.set_terrain(*x, *y, *code, FLOOR);
    }
    cdda_sim::load_level(&mut world, level);

    let mut ground = TerrainTable::new();
    ground.insert(FLOOR, true);
    ground.insert(WALL, false);
    let mut furniture = TerrainTable::new();
    furniture.insert(0, true); // the null furniture is see-through
    let mut fields = FieldTable::new();
    // The numbers are the loaded ones: `fd_smoke` is translucency 1, 10 and 0 at its three intensities (`field_type.json`).
    fields.insert(SMOKE, &[1.0, 10.0, 0.0]);
    cdda_sim::set_prototypes(&mut world, Prototypes {
        terrain: ground,
        furniture,
    });
    set_field_table(&mut world, fields);
    world
}

#[test]
fn an_open_room_is_in_sight_across() {
    let world = world(&[]);
    assert!(sees(&world, (1, 1), (9, 9), 60));
}

#[test]
fn a_wall_between_two_squares_stops_the_sight() {
    let world = world(&[(5, 5, WALL)]);
    assert!(!sees(&world, (1, 1), (9, 9), 60));
}

#[test]
fn a_wall_that_is_the_target_is_still_seen() {
    let world = world(&[(9, 9, WALL)]);
    assert!(sees(&world, (1, 1), (9, 9), 60));
}

#[test]
fn a_solid_field_blinds_the_line_and_a_dimming_one_does_not() {
    // `is_transparent` counts the fields and `is_transparent_wo_fields` does not (`src/map.cpp`), so the same geometry answers two ways depending on which the caller asks
    // for. Smoke's own data says intensity one has a translucency of one and intensity three has zero (`field_type.json`), so the same field blinds or dims depending on
    // how thick it is - which is why the port reads the number off the loaded prototypes rather than deciding for itself.
    let smoke = |intensity: i32| {
        let mut world = world(&[]);
        let mut entry = FieldEntry::new(3, 0, false, false);
        entry.set_intensity(intensity);
        world.spawn((
            Position { x: 5, y: 5, z: 0 },
            Field {
                id: FieldId(SMOKE),
                entry,
                priority: 0,
            },
        ));
        world
    };

    assert!(!sees(&smoke(3), (1, 1), (9, 9), 60), "thick smoke is solid");
    assert!(
        sees(&smoke(1), (1, 1), (9, 9), 60),
        "thin smoke only dims what is behind it"
    );
}
