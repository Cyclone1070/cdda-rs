//! The acid vent: thinning with age, and refilling when spent.
//!
//! Transcribed from `field_processor_fd_acid_vent` (`src/map_field.cpp:877-901`). The reference's
//! acid-revival branch is dead code - it looks for fields at intensity zero, and neither
//! implementation can hold one - so the port keeps only what can happen, and the divergence is in
//! the ledger rather than hidden.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::rng::Rolls;
use cdda_sim::{ACID_VENT, Field, Position, process_acid_vent};

fn spawn(world: &mut World, intensity: i32, age: i64) -> Entity {
    let mut entry = FieldEntry::new(3, 100, true, false);
    entry.set_intensity(intensity);
    entry.set_age(age);
    world
        .spawn((
            Position { x: 5, y: 5 },
            Field {
                id: ACID_VENT,
                entry,
                priority: 0,
            },
        ))
        .id()
}

fn field(world: &mut World, entity: Entity) -> (i32, i64) {
    world
        .get::<Field>(entity)
        .map_or((0, 0), |field| (field.entry.intensity(), field.entry.age()))
}

#[test]
fn a_young_vent_at_full_strength_is_unchanged() {
    let mut world = World::new();
    let vent = spawn(&mut world, 2, 30);
    let mut rolls = Rolls::new(1);
    process_acid_vent(&mut world, vent, &mut rolls);
    assert_eq!(field(&mut world, vent), (2, 30), "still 2, still young");
}

#[test]
fn a_minute_old_vent_thins_and_its_age_starts_again() {
    let mut world = World::new();
    let vent = spawn(&mut world, 2, 60);
    let mut rolls = Rolls::new(1);
    process_acid_vent(&mut world, vent, &mut rolls);
    assert_eq!(
        field(&mut world, vent),
        (1, 0),
        "one step weaker, age reset"
    );
}

#[test]
fn a_spent_vent_refills_to_full_strength() {
    let mut world = World::new();
    let vent = spawn(&mut world, 1, 0);
    let mut rolls = Rolls::new(1);
    process_acid_vent(&mut world, vent, &mut rolls);
    assert_eq!(field(&mut world, vent).0, 3, "a spent vent is full again");
}

#[test]
fn a_vent_never_makes_acid_of_its_own() {
    let mut world = World::new();
    let vent = spawn(&mut world, 1, 0);
    let mut rolls = Rolls::new(1);
    process_acid_vent(&mut world, vent, &mut rolls);
    let mut query = world.query::<(&Field, &Position)>();
    assert_eq!(
        query.iter(&world).count(),
        1,
        "the vent itself and nothing else: the revival branch is dead in both implementations"
    );
}

#[test]
fn an_old_vent_at_one_still_refills_rather_than_thinning() {
    let mut world = World::new();
    let vent = spawn(&mut world, 1, 600);
    let mut rolls = Rolls::new(1);
    process_acid_vent(&mut world, vent, &mut rolls);
    assert_eq!(
        field(&mut world, vent).0,
        3,
        "the refill is unconditional at intensity 1"
    );
}
