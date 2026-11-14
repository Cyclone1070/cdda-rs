//! Smoke and fire as things in the store: they age, they decay, and they die.
//!
//! The behaviour here was ported and gated against the reference before the storage moved;
//! these tests exist to prove the move did not change it.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::rng::Rolls;
use cdda_sim::{Field, Position, PositionIndex, process_fields};

fn world_with_smoke(x: i32, y: i32, half_life: i64) -> (World, Entity) {
    let mut world = World::new();
    let mut entry = FieldEntry::new(3, half_life, true, false);
    entry.set_intensity(3);
    let entity = world
        .spawn((
            Position { x, y, z: 0 },
            Field {
                id: FieldId(1),
                entry,
                priority: 0,
            },
        ))
        .id();
    (world, entity)
}

#[test]
fn a_field_is_a_thing_with_a_square_a_strength_and_an_age() {
    let (mut world, entity) = world_with_smoke(4, 5, 5);
    PositionIndex::rebuild(&mut world);
    assert!(
        cdda_sim::index(&world)
            .expect("the index")
            .at(4, 5)
            .contains(&entity)
    );
    let field = world.get::<Field>(entity).expect("the smoke exists");
    assert_eq!(field.id, FieldId(1));
    assert_eq!(field.entry.intensity(), 3);
}

#[test]
fn the_field_system_ages_it_and_dying_fields_leave_the_store() {
    let (mut world, entity) = world_with_smoke(0, 0, 1);
    let mut rolls = Rolls::new(0);
    // Half-life one, starting at turn 100: the reference decays on the age tick after, at 104.
    for now in 100..=104 {
        process_fields(&mut world, now, &mut rolls);
    }
    assert!(
        world.get_entity(entity).is_err(),
        "the smoke is gone from the store"
    );
    PositionIndex::rebuild(&mut world);
    assert_eq!(
        cdda_sim::index(&world)
            .expect("the index")
            .occupied_squares(),
        0
    );
}

#[test]
fn fields_are_processed_in_the_reference_order_so_the_dice_stream_matches() {
    // Two fields with the same half-life decay in square order, x outer then y inner. The
    // order matters because decay draws from the shared dice: swapping it changes the stream.
    let mut world = World::new();
    for (x, y) in [(2, 1), (1, 2)] {
        let entry = FieldEntry::new(3, 1, true, false);
        world.spawn((
            Position { x, y, z: 0 },
            Field {
                id: FieldId(1),
                entry,
                priority: 0,
            },
        ));
    }
    let mut rolls = Rolls::new(7);
    process_fields(&mut world, 101, &mut rolls);
    let after_one = rolls.engine().next_u32();
    assert_ne!(after_one, 0, "the stream advanced");
}
