//! One gas rule, whichever door a caller comes in by.
//!
//! The port grew two gas-spread implementations: the session sweep's, which follows `map::spread_gas` (`src/map_field.cpp:276-368`) including the square below and
//! the square above, and an older per-field one that only ever looked sideways. Two rules that disagree are worse than one rule that is wrong, because the session
//! gates were green while the older path quietly dropped every fall. These tests hold the older door to the same rule: down first, then the neighbours, then up.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::rng::{Recording, Rolls};
use cdda_core_types::field::FieldId;
use cdda_sim::{Field, GasContext, Position, spread_gas};

/// A gas, as the test file's own id: the rule does not care which field it is.
const GAS: FieldId = FieldId(20);

fn spawn(world: &mut World, z: i32, intensity: i32, age: i64) -> Entity {
    let mut entry = FieldEntry::new(3, 1_000_000, false, false);
    entry.set_intensity(intensity);
    entry.set_age(age);
    world
        .spawn((
            Position { x: 5, y: 5, z },
            Field {
                id: GAS,
                entry,
                priority: 0,
            },
        ))
        .id()
}

/// Every field in the world, sorted, as `(x, y, z, intensity, age)`.
fn fields(world: &mut World) -> Vec<(i32, i32, i32, i32, i64)> {
    let mut query = world.query::<(&Position, &Field)>();
    let mut found: Vec<(i32, i32, i32, i32, i64)> = query
        .iter(world)
        .map(|(position, field)| {
            (
                position.x,
                position.y,
                position.z,
                field.entry.intensity(),
                field.entry.age(),
            )
        })
        .collect();
    found.sort_unstable();
    found
}

fn still() -> GasContext {
    GasContext {
        percent_spread: 30,
        windpower: 0,
        wind_angle: 0,
        outdoors: false,
        outdoor_age_speedup: 0,
    }
}

#[test]
fn the_older_door_falls_before_it_looks_sideways() {
    // One die: the spread chance. Nothing else may be spent, because the reference tries the square below first and for free.
    let mut world = World::new();
    let gas = spawn(&mut world, 0, 3, 5);
    let mut rolls = Rolls::replaying(Recording::parse("int 1 100 5\n").expect("one die"));
    let context = still();
    spread_gas(&mut world, gas, &context, |_from, to| (to.0, to.1) != (6, 5), &mut rolls);
    assert_eq!(
        rolls.recording_remaining(),
        Some(0),
        "the chance die is the only one a fall spends"
    );
    assert_eq!(
        fields(&mut world),
        vec![(5, 5, -1, 1, 1), (5, 5, 0, 2, 4)],
        "the gas fell, and took a share of its age with it"
    );
}

#[test]
fn gas_still_only_goes_sideways_when_the_floor_holds_it() {
    // No square below to take it, so the lateral scan runs: the start neighbour, whether it goes, and which one.
    let mut world = World::new();
    let gas = spawn(&mut world, 0, 3, 5);
    let recording = Recording::parse("int 1 100 5\nint 0 7 0\nint 0 7 0\nint 0 7 0\n").expect("four dice");
    let mut rolls = Rolls::replaying(recording);
    let context = still();
    // A floor under the gas, so the fall is not on the table and the lateral scan is what runs.
    spread_gas(&mut world, gas, &context, |from, to| to.2 == from.2, &mut rolls);
    let squares = fields(&mut world);
    assert!(
        squares.iter().any(|&(_, _, z, intensity, _)| z == 0 && intensity == 1),
        "the share went to a neighbour: {squares:?}"
    );
}
