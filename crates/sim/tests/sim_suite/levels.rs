//! Levels: the same column, different floors.
//!
//! The reference's map is three dimensional and its rules depend on it - gas falls before it drifts, smoke rises, a fire on one floor does not thicken the
//! smoke on the next. These tests are about the store keeping floors apart at all, which has to be true before any of those rules can be right.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_sim::{Field, Position, PositionIndex, SMOKE, add_field_strength};

fn smoke(world: &mut World, x: i32, y: i32, z: i32, intensity: i32) -> Entity {
    let mut entry = FieldEntry::new(3, 1_000_000, false, false);
    entry.set_intensity(intensity);
    world
        .spawn((
            Field {
                id: SMOKE,
                entry,
                priority: 0,
            },
            Position { x, y, z },
        ))
        .id()
}

fn intensity_at(world: &mut World, x: i32, y: i32, z: i32) -> Vec<i32> {
    let mut query = world.query::<(&Field, &Position)>();
    let mut found: Vec<i32> = query
        .iter(world)
        .filter(|(field, position)| {
            field.id == SMOKE && position.x == x && position.y == y && position.z == z
        })
        .map(|(field, _)| field.entry.intensity())
        .collect();
    found.sort_unstable();
    found
}

#[test]
fn the_index_keeps_floors_apart() {
    // Same column, two floors: the index must answer for the floor it was asked about, not for the column.
    let mut world = World::new();
    let ground = smoke(&mut world, 3, 4, 0, 2);
    let cellar = smoke(&mut world, 3, 4, -1, 1);
    PositionIndex::rebuild(&mut world);
    let index = world.resource::<PositionIndex>();
    assert_eq!(index.at_level(3, 4, 0), &[ground]);
    assert_eq!(index.at_level(3, 4, -1), &[cellar]);
    assert_eq!(index.at(3, 4), &[ground], "`at` is street level");
    assert!(index.at_level(3, 4, 1).is_empty());
}

#[test]
fn adding_smoke_on_one_floor_does_not_thicken_the_smoke_below() {
    // The merge rule adds to a field of the same kind on the same square. On a different floor it is a different square, and a fire upstairs must not
    // feed the smoke in the cellar.
    let mut world = World::new();
    smoke(&mut world, 5, 5, 0, 2);
    add_field_strength(&mut world, (5, 5, -1), SMOKE, 1);
    assert_eq!(
        intensity_at(&mut world, 5, 5, 0),
        vec![2],
        "the floor above is untouched"
    );
    assert_eq!(
        intensity_at(&mut world, 5, 5, -1),
        vec![1],
        "and the cellar got its own field"
    );
}

#[test]
fn adding_smoke_on_the_same_floor_still_merges() {
    // The counterpart: the same floor still adds up, or the test above would pass for the wrong reason.
    let mut world = World::new();
    smoke(&mut world, 5, 5, 0, 2);
    add_field_strength(&mut world, (5, 5, 0), SMOKE, 1);
    assert_eq!(intensity_at(&mut world, 5, 5, 0), vec![3]);
}
