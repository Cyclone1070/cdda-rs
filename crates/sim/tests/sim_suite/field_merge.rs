//! What happens when a field lands on a square that already has one of its kind.
//!
//! Reference: `field::add_field` (`src/field.cpp:158-175`). When the type is already there the reference does **not** keep the stronger of the two:
//! it writes `prev_intensity + new_intensity`, and the setter clamps that to the type's maximum. Two patches of smoke one and two thick make three,
//! not two - and a port that took the maximum would leave a room half as smoky as the reference's.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_sim::{Field, Position, SMOKE};

fn smoke(world: &mut World, x: i32, y: i32, intensity: i32) -> Entity {
    let mut entry = FieldEntry::new(3, 100, true, false);
    entry.set_intensity(intensity);
    world
        .spawn((
            Field {
                id: SMOKE,
                entry,
                priority: 0,
            },
            Position { x, y },
        ))
        .id()
}

#[test]
fn two_patches_of_smoke_add_up() {
    let mut world = World::new();
    let entity = smoke(&mut world, 3, 3, 1);
    cdda_sim::add_field_strength(&mut world, (3, 3), SMOKE, 2);
    let field = world.get::<Field>(entity).expect("the smoke");
    assert_eq!(field.entry.intensity(), 3, "one and two make three");
}

#[test]
fn the_sum_is_clamped_to_what_the_field_can_be() {
    // Intensities stop at the type's maximum; the reference's setter clamps, and so does this one.
    let mut world = World::new();
    let entity = smoke(&mut world, 3, 3, 2);
    cdda_sim::add_field_strength(&mut world, (3, 3), SMOKE, 2);
    assert_eq!(
        world
            .get::<Field>(entity)
            .expect("the smoke")
            .entry
            .intensity(),
        3
    );
}

#[test]
fn a_field_on_an_empty_square_is_created_with_the_strength_it_was_given() {
    let mut world = World::new();
    cdda_sim::add_field_strength(&mut world, (5, 5), SMOKE, 2);
    let mut query = world.query::<(&Field, &Position)>();
    let found: Vec<i32> = query
        .iter(&world)
        .filter(|(field, position)| field.id == SMOKE && position.x == 5 && position.y == 5)
        .map(|(field, _)| field.entry.intensity())
        .collect();
    assert_eq!(found, vec![2]);
}
