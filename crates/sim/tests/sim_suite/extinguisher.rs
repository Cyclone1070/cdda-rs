//! An extinguisher cloud and a fire on the same square.
//!
//! Transcribed from `field_processor_fd_extinguisher` (`src/map_field.cpp:559-569`): both lose the
//! smaller of the two strengths, so the weaker one is spent and the stronger one survives smaller.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_sim::{EXTINGUISHER, FIRE, Field, Position, process_extinguisher};

fn spawn(world: &mut World, id: FieldId, intensity: i32, at: (i32, i32)) -> Entity {
    let mut entry = FieldEntry::new(3, 100, true, false);
    entry.set_intensity(intensity);
    world
        .spawn((
            Position {
                x: at.0,
                y: at.1,
                z: 0,
            },
            Field {
                id,
                entry,
                priority: 0,
            },
        ))
        .id()
}

fn intensity(world: &mut World, entity: Entity) -> Option<i32> {
    world
        .get::<Field>(entity)
        .map(|field| field.entry.intensity())
}

#[test]
fn a_stronger_cloud_puts_the_fire_out_and_survives_smaller() {
    let mut world = World::new();
    let fire = spawn(&mut world, FIRE, 1, (5, 5));
    let cloud = spawn(&mut world, EXTINGUISHER, 3, (5, 5));
    process_extinguisher(&mut world, cloud);
    assert_eq!(intensity(&mut world, fire), None, "the fire is gone");
    assert_eq!(
        intensity(&mut world, cloud),
        Some(2),
        "the cloud spent one of its three"
    );
}

#[test]
fn a_weaker_cloud_weakens_the_fire_and_is_spent() {
    let mut world = World::new();
    let fire = spawn(&mut world, FIRE, 3, (5, 5));
    let cloud = spawn(&mut world, EXTINGUISHER, 1, (5, 5));
    process_extinguisher(&mut world, cloud);
    assert_eq!(
        intensity(&mut world, fire),
        Some(2),
        "the fire burned lower"
    );
    assert_eq!(intensity(&mut world, cloud), None, "the cloud is used up");
}

#[test]
fn an_equal_cloud_and_fire_remove_each_other() {
    let mut world = World::new();
    let fire = spawn(&mut world, FIRE, 2, (5, 5));
    let cloud = spawn(&mut world, EXTINGUISHER, 2, (5, 5));
    process_extinguisher(&mut world, cloud);
    assert_eq!(intensity(&mut world, fire), None);
    assert_eq!(intensity(&mut world, cloud), None);
}

#[test]
fn a_cloud_with_no_fire_does_nothing() {
    let mut world = World::new();
    let cloud = spawn(&mut world, EXTINGUISHER, 2, (5, 5));
    process_extinguisher(&mut world, cloud);
    assert_eq!(
        intensity(&mut world, cloud),
        Some(2),
        "an idle cloud keeps its strength"
    );
}

#[test]
fn fire_on_a_neighbouring_square_is_not_touched() {
    let mut world = World::new();
    let fire = spawn(&mut world, FIRE, 3, (6, 5));
    let cloud = spawn(&mut world, EXTINGUISHER, 3, (5, 5));
    process_extinguisher(&mut world, cloud);
    assert_eq!(
        intensity(&mut world, fire),
        Some(3),
        "a cloud fights only where it is"
    );
    assert_eq!(intensity(&mut world, cloud), Some(3));
}
