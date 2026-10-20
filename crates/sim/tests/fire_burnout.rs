//! Fire over bare ground dies faster, and a strong fire dies faster still.
//!
//! Transcribed from `field_processor_fd_fire` (`src/map_field.cpp:1128-1132`): with nothing burned
//! this turn, a fire whose tile has no fuel gains ten seconds of age per point of intensity.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_sim::{FIRE, Field, Position, process_fire_burnout};

fn burning(world: &mut World, intensity: i32, age: i64) -> Entity {
    let mut entry = FieldEntry::new(3, 100, true, true);
    entry.set_intensity(intensity);
    entry.set_age(age);
    world
        .spawn((
            Position { x: 5, y: 5 },
            Field {
                id: FIRE,
                entry,
                priority: 0,
            },
        ))
        .id()
}

fn age(world: &mut World, entity: Entity) -> i64 {
    world
        .get::<Field>(entity)
        .map_or(-1, |field| field.entry.age())
}

#[test]
fn a_fire_over_bare_ground_ages_ten_seconds_per_point_of_intensity() {
    for (intensity, expected) in [(1, 10), (2, 20), (3, 30)] {
        let mut world = World::new();
        let fire = burning(&mut world, intensity, 0);
        process_fire_burnout(&mut world, fire, false);
        assert_eq!(age(&mut world, fire), expected, "intensity {intensity}");
    }
}

#[test]
fn a_fire_with_fuel_does_not_gain_age_this_way() {
    let mut world = World::new();
    let fire = burning(&mut world, 3, 0);
    process_fire_burnout(&mut world, fire, true);
    assert_eq!(
        age(&mut world, fire),
        0,
        "fuel means the fire is not starving"
    );
}

#[test]
fn age_piles_up_turn_after_turn() {
    let mut world = World::new();
    let fire = burning(&mut world, 2, 0);
    for _ in 0..3 {
        process_fire_burnout(&mut world, fire, false);
    }
    assert_eq!(age(&mut world, fire), 60, "three turns of twenty seconds");
}

#[test]
fn a_fire_that_is_gone_is_left_alone() {
    let mut world = World::new();
    let fire = burning(&mut world, 1, 0);
    world.despawn(fire);
    process_fire_burnout(&mut world, fire, false);
    assert_eq!(
        world.get::<Field>(fire).map(|_| ()),
        None,
        "despawning twice would panic"
    );
}
