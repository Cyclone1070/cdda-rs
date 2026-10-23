//! Which squares scent cannot cross, with the answer coming from the loaded terrain flags.
//!
//! Source: the two flags `scent_map::update` reads, `NO_SCENT` and `REDUCE_SCENT` (`src/mapdata.h:181-193`). The diffusion arithmetic is `cdda_core_types::scent::ScentWindow`,
//! which is pinned by its own tests; the question here is whether a wall flags *itself*. The port used to make the caller mark squares by hand, which let a wall's flag and the
//! mark disagree.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_sim::scent::{decay_scent, marks_at, reset_scent, scent_at, set_scent_at};
use cdda_sim::scent_table::ScentTable;
use cdda_sim::{Map, Prototypes};

const AIR: u32 = 1;
const WALL: u32 = 2; // NO_SCENT
const DOOR: u32 = 3; // REDUCE_SCENT

fn world(marks: &[(i32, i32, u32)]) -> World {
    let mut world = World::new();
    let mut level = Level::new();
    level.ensure(0, 0, AIR);
    for y in 0..12 {
        for x in 0..12 {
            level.set_terrain(x, y, AIR, AIR);
        }
    }
    for (x, y, code) in marks {
        level.set_terrain(*x, *y, *code, AIR);
    }
    world.insert_resource(Map(level));
    world.insert_resource(Prototypes::default());

    let mut scent = ScentTable::new();
    scent.insert(AIR, false, false);
    scent.insert(WALL, true, false);
    scent.insert(DOOR, false, true);
    world.insert_resource(scent);
    reset_scent(&mut world);
    world
}

#[test]
fn a_no_scent_wall_marks_its_own_square_and_plain_air_marks_nothing() {
    // `NO_SCENT` acts like a wall for scent (`mapdata.h:181`): nothing passes in or out, and the square itself is emptied. The mark comes from the terrain code through the
    // loaded flags, so a wall cannot be forgotten by a caller that never marked it.
    let world = world(&[(6, 5, WALL)]);
    assert_eq!(marks_at(&world, (6, 5)), (true, false), "the wall blocks");
    assert_eq!(marks_at(&world, (5, 5)), (false, false), "air does not");
}

#[test]
fn a_reduce_scent_door_marks_its_square_as_reducing() {
    // `REDUCE_SCENT` takes part at a fifth of the weight (`mapdata.h:193`), and the two marks are independent: a door reduces, it does not block.
    let world = world(&[(6, 5, DOOR)]);
    assert_eq!(marks_at(&world, (6, 5)), (false, true));
    assert_eq!(marks_at(&world, (5, 5)), (false, false));
}

#[test]
fn the_marks_come_from_the_table_rather_than_from_the_code_number() {
    // The same terrain code answers differently under a different table, which is what "loaded from the data" has to mean - otherwise the port would only have moved the
    // hard-coded rule from a match arm into the sim.
    let mut reversed = world(&[(6, 5, AIR)]);
    let mut table = ScentTable::new();
    table.insert(AIR, true, false);
    reversed.insert_resource(table);
    reset_scent(&mut reversed);
    assert_eq!(marks_at(&reversed, (6, 5)), (true, false));
}

#[test]
fn scent_fades_by_one_a_turn_and_never_below_zero() {
    // `scent_map::decay` (`scent_map.cpp:60-67`) takes one point off every square and floors at zero. It is a separate verb from the diffusion because the reference calls it
    // separately, once a turn, before the fields are processed (`map.cpp:3417-3420`).
    let mut world = world(&[]);
    set_scent_at(&mut world, (5, 5), 3);
    decay_scent(&mut world);
    assert_eq!(scent_at(&world, (5, 5)), 2);
    for _ in 0..5 {
        decay_scent(&mut world);
    }
    assert_eq!(scent_at(&world, (5, 5)), 0, "and it stops at nothing");
}
