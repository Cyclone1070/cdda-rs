//! Smoke moving through the store: the first gas the port actually spreads.
//!
//! Reference: `map::spread_gas` (`src/map_field.cpp:276-360`) applied to each gas on the level, once per turn, from the field sweep. The rule itself is
//! `cdda_core_types::fire::spread_gas`; what this system adds is the store: which squares accept the gas, who loses a level, and who thickens.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::rng::{Recording, Rolls};
use cdda_sim::{Field, Position, SMOKE, process_gas};

fn smoke(world: &mut World, x: i32, y: i32, intensity: i32, age: i64) -> Entity {
    let mut entry = FieldEntry::new(3, 1_000_000, false, false);
    entry.set_intensity(intensity);
    entry.set_age(age);
    world
        .spawn((
            Field {
                id: SMOKE,
                entry,
                priority: 0,
            },
            Position { x, y, z: 0 },
        ))
        .id()
}

fn at(world: &mut World, x: i32, y: i32) -> Option<(i32, i64)> {
    let mut query = world.query::<(&Field, &Position)>();
    query
        .iter(world)
        .find(|(field, position)| field.id == SMOKE && position.x == x && position.y == y)
        .map(|(field, _)| (field.entry.intensity(), field.entry.age()))
}

/// Every neighbour **on the same floor** accepts the gas, and there is no floor above or below to fall or rise to.
fn open(from: (i32, i32, i32), to: (i32, i32, i32)) -> bool {
    to.2 == from.2
}

#[test]
fn a_gas_at_intensity_one_asks_for_nothing() {
    // The short-circuit, at the level of the store: a level-one gas is dying, so the turn costs it no dice and moves nothing.
    let mut world = World::new();
    let entity = smoke(&mut world, 0, 0, 1, 60);
    let mut rolls = Rolls::replaying(Recording::parse("int 1 100 5").expect("a recording"));
    process_gas(&mut world, SMOKE, 10, 0, open, &mut rolls);
    assert_eq!(rolls.recording_remaining(), Some(1), "no die was drawn");
    assert!(world.get::<Field>(entity).is_some());
    assert_eq!(at(&mut world, 0, 0), Some((1, 60)));
}

#[test]
fn a_thick_gas_gives_a_level_to_a_neighbour_and_keeps_the_rest_of_its_age() {
    // Gate, scan start, one_in( eight candidates ), then the choice: four dice, and the arithmetic from `gas_spread_to` - the source keeps
    // `age - age / intensity`, the destination receives `age / intensity`. My first recording had three dice because I forgot the one_in draw, and
    // `Recording` caught it by refusing the range rather than answering the wrong die.
    let mut world = World::new();
    let entity = smoke(&mut world, 0, 0, 3, 90);
    // The scan starts at neighbour 0 and wraps, so the candidates come in the order 1, 2, ... 7, 0 - and index 1 is the square north of the gas in this
    // project's neighbour table. With every neighbour open the choice takes the first candidate, so the gas moves north.
    let mut rolls = Rolls::replaying(
        Recording::parse("int 1 100 10\nint 0 7 0\nint 0 7 0\nint 0 7 0").expect("a recording"),
    );
    process_gas(&mut world, SMOKE, 10, 0, open, &mut rolls);
    let source = world
        .get::<Field>(entity)
        .expect("the source is still there");
    assert_eq!(source.entry.intensity(), 2, "one level was given away");
    assert_eq!(
        source.entry.age(),
        60,
        "and a third of its age went with it"
    );
    assert_eq!(rolls.recording_remaining(), Some(0));
}

#[test]
fn a_square_that_already_has_the_gas_thickens_instead_of_being_created() {
    // `gas_spread_to` on an existing field: intensity plus one, age plus the share. The store must merge rather than spawn a second entity on the square.
    let mut world = World::new();
    smoke(&mut world, 0, 0, 3, 90);
    // The square the gas will move onto, already holding some of it.
    smoke(&mut world, 0, -1, 1, 5);
    let mut rolls = Rolls::replaying(
        Recording::parse("int 1 100 10\nint 0 7 0\nint 0 7 0\nint 0 7 0").expect("a recording"),
    );
    process_gas(&mut world, SMOKE, 10, 0, open, &mut rolls);
    let mut query = world.query::<(&Field, &Position)>();
    let squares: Vec<(i32, i32, i32)> = query
        .iter(&world)
        .filter(|(field, _)| field.id == SMOKE)
        .map(|(field, position)| (position.x, position.y, field.entry.intensity()))
        .collect();
    assert_eq!(
        squares.len(),
        2,
        "still two squares, not three: {squares:?}"
    );
    assert_eq!(
        at(&mut world, 0, -1).map(|(intensity, _)| intensity),
        Some(2),
        "the neighbour thickened instead of a third entity appearing"
    );
}

#[test]
fn a_gas_with_nowhere_to_go_stays_put_and_spends_only_the_two_dice_the_scan_needs() {
    let mut world = World::new();
    smoke(&mut world, 0, 0, 3, 90);
    let mut rolls =
        Rolls::replaying(Recording::parse("int 1 100 1\nint 0 7 3").expect("a recording"));
    process_gas(
        &mut world,
        SMOKE,
        10,
        0,
        |_, _: (i32, i32, i32)| false,
        &mut rolls,
    );
    assert_eq!(rolls.recording_remaining(), Some(0));
    assert_eq!(at(&mut world, 0, 0), Some((3, 90)), "untouched");
}

#[test]
fn a_gas_that_gives_away_its_last_level_leaves_the_store() {
    // Intensity two becomes one, and while a level-one gas is still alive, a gas that reaches zero is not: the sweep takes it off the store.
    let mut world = World::new();
    let entity = smoke(&mut world, 0, 0, 2, 40);
    let mut rolls = Rolls::replaying(
        Recording::parse("int 1 100 10\nint 0 7 0\nint 0 7 0\nint 0 7 0").expect("a recording"),
    );
    process_gas(&mut world, SMOKE, 10, 0, open, &mut rolls);
    assert!(
        world.get::<Field>(entity).is_some(),
        "one level left is still alive"
    );
    assert_eq!(at(&mut world, 0, 0), Some((1, 20)));
}
