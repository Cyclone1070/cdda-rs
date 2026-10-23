//! The incendiary field: two dice pick a neighbouring square, and flammability decides whether it catches.
//!
//! Transcribed from `field_processor_fd_incendiary` (`src/map_field.cpp:928-946`). The processor is small and every line of it is a decision worth pinning: the
//! square it looks at is drawn **before** anything else happens, it can be the field's own square, and the hot air is made whether or not anything caught. The
//! item half (`flammable_items_at`) belongs to the item layer, which does not exist yet, so it is recorded as a gap rather than guessed at.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::rng::{Recording, Rolls};
use cdda_core_types::field::FieldId;
use cdda_sim::{FIRE, Field, Position, TerrainFacts, process_incendiary};

/// The incendiary field, as the registry numbers it.
const INCENDIARY: FieldId = FieldId(30);

fn spawn(world: &mut World, intensity: i32) -> Entity {
    let mut entry = FieldEntry::new(4, 100, true, false);
    entry.set_intensity(intensity);
    world
        .spawn((
            Position { x: 5, y: 5, z: 0 },
            Field {
                id: INCENDIARY,
                entry,
                priority: 0,
            },
        ))
        .id()
}

/// The facts of a square: only what the rule reads.
fn dry() -> TerrainFacts {
    TerrainFacts::default()
}

fn grass() -> TerrainFacts {
    TerrainFacts {
        flammable: true,
        ..TerrainFacts::default()
    }
}

/// Every field in the world, sorted, as `(x, y, z, id, intensity)`.
fn fields(world: &mut World) -> Vec<(i32, i32, i32, u32, i32)> {
    let mut query = world.query::<(&Position, &Field)>();
    let mut found: Vec<(i32, i32, i32, u32, i32)> = query
        .iter(world)
        .map(|(position, field)| {
            (
                position.x,
                position.y,
                position.z,
                field.id.0,
                field.entry.intensity(),
            )
        })
        .collect();
    found.sort_unstable();
    found
}

#[test]
fn a_flammable_neighbour_catches_fire_and_the_dice_come_first() {
    let mut world = World::new();
    let entity = spawn(&mut world, 2);
    // Two draws pick the square: (-1, 1) from the field at (5,5) is (4,6). Then `create_hot_air` spends five pairs, all of which land on the field's own square here,
    // so the five wisps are one field - at intensity three, not five, because the type has three levels and the reference's `add_field` stops at its own maximum.
    let recording = Recording::parse(
        "int -1 1 -1\nint -1 1 1\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\n",
    )
    .expect("twelve dice");
    let mut rolls = Rolls::replaying(recording);
    let facts = |x: i32, y: i32, _z: i32| {
        if (x, y) == (4, 6) {
            grass()
        } else {
            dry()
        }
    };
    process_incendiary(&mut world, entity, facts, &mut rolls);
    assert_eq!(rolls.recording_remaining(), Some(0), "every die was spent");
    assert_eq!(
        fields(&mut world),
        vec![
            (4, 6, 0, FIRE.0, 1),
            (5, 5, 0, INCENDIARY.0, 2),
            (5, 5, 0, 59, 3),
        ],
        "the neighbour caught fire and the hot air is capped at the type's three levels"
    );
}

#[test]
fn a_neighbour_that_cannot_burn_gets_no_fire_but_the_hot_air_still_comes() {
    let mut world = World::new();
    let entity = spawn(&mut world, 2);
    let recording = Recording::parse(
        "int -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 1\nint -1 1 1\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\n",
    )
    .expect("twelve dice");
    let mut rolls = Rolls::replaying(recording);
    // (0,0) is the field's own square, which is where the reference puts the fire when the dice land there.
    process_incendiary(&mut world, entity, |_x, _y, _z| dry(), &mut rolls);
    assert_eq!(
        fields(&mut world),
        vec![
            (5, 5, 0, INCENDIARY.0, 2),
            (5, 5, 0, 59, 3),
            (6, 6, 0, 59, 1),
        ],
        "nothing burned, and the hot air is unconditional"
    );
}

#[test]
fn the_dice_are_drawn_even_when_the_field_is_about_to_die() {
    // The reference draws the two offsets before it asks anything about the square, so a field with one intensity left still spends them. A port that looked at
    // the square first would take the same branch and shift every later die in the session.
    let mut world = World::new();
    let entity = spawn(&mut world, 1);
    let recording = Recording::parse(
        "int -1 1 1\nint -1 1 1\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\nint -1 1 0\n",
    )
    .expect("twelve dice");
    let mut rolls = Rolls::replaying(recording);
    process_incendiary(&mut world, entity, |_x, _y, _z| grass(), &mut rolls);
    assert_eq!(rolls.recording_remaining(), Some(0), "both dice were spent");
}
