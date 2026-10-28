//! Two small processors: a field dissipating underwater, and one growing on its own.
//!
//! Transcribed from `field_processor_underwater_dissipation` and `field_processor_upgrade_intensity`
//! (`src/map_field.cpp:503-523`).

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::rng::Rolls;
use cdda_sim::{Field, Position, process_underwater_dissipation, process_upgrade_intensity};

fn field(world: &mut World, intensity: i32, age: i64) -> Entity {
    let mut entry = FieldEntry::new(3, 100, true, false);
    entry.set_intensity(intensity);
    entry.set_age(age);
    world
        .spawn((
            Position { x: 5, y: 5, z: 0 },
            Field {
                id: FieldId(1),
                entry,
                priority: 0,
            },
        ))
        .id()
}

#[test]
fn a_field_under_water_ages_faster() {
    let mut world = World::new();
    let entity = field(&mut world, 2, 0);
    process_underwater_dissipation(&mut world, entity, true, 40);
    let age = world.get::<Field>(entity).map_or(0, |f| f.entry.age());
    assert_eq!(age, 40);
}

#[test]
fn a_field_on_dry_land_does_not() {
    let mut world = World::new();
    let entity = field(&mut world, 2, 0);
    process_underwater_dissipation(&mut world, entity, false, 40);
    let age = world.get::<Field>(entity).map_or(0, |f| f.entry.age());
    assert_eq!(age, 0);
}

#[test]
fn a_field_that_grows_does_so_on_its_period_and_sometimes_after() {
    let mut grew = 0;
    let mut stayed = 0;
    for attempt in 1..=40 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut world = World::new();
        let entity = field(&mut world, 1, 0);
        let mut rolls = Rolls::new(seed);
        process_upgrade_intensity(&mut world, entity, 100, 5, 50, &mut rolls);
        let intensity = world
            .get::<Field>(entity)
            .map_or(0, |f| f.entry.intensity());
        if intensity == 2 {
            grew += 1;
        } else {
            stayed += 1;
        }
    }
    assert!(
        grew > 0 && stayed > 0,
        "{grew} grew, {stayed} stayed over 40 seeds"
    );
}

#[test]
fn off_its_period_nothing_is_drawn() {
    let mut world = World::new();
    let entity = field(&mut world, 1, 0);
    let mut rolls = Rolls::new(1);
    let mut control = Rolls::new(1);
    process_upgrade_intensity(&mut world, entity, 101, 5, 50, &mut rolls);
    assert_eq!(
        rolls.rng(1, 1000),
        control.rng(1, 1000),
        "no dice off the period"
    );
    assert_eq!(
        world
            .get::<Field>(entity)
            .map_or(0, |f| f.entry.intensity()),
        1
    );
}

#[test]
fn a_field_with_no_upgrade_chance_never_grows() {
    let mut world = World::new();
    let entity = field(&mut world, 2, 0);
    let mut rolls = Rolls::new(1);
    let mut control = Rolls::new(1);
    process_upgrade_intensity(&mut world, entity, 0, 0, 50, &mut rolls);
    assert_eq!(
        rolls.rng(1, 1000),
        control.rng(1, 1000),
        "chance zero draws nothing"
    );
    assert_eq!(
        world
            .get::<Field>(entity)
            .map_or(0, |f| f.entry.intensity()),
        2
    );
}
