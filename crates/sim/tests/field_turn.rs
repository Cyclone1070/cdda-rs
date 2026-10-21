//! One turn over every field in the world, in the reference's order.
//!
//! Transcribed from `map::process_fields_in_submap` (`src/map_field.cpp:160-460`): age, dissipate,
//! upgrade, spread - and a field that aged away is gone before the later steps run.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::{FieldEntry, FieldId};
use cdda_core_types::rng::Rolls;
use cdda_sim::{CellFacts, Field, FieldTurnData, Position, process_field_turn};

fn field_at(world: &mut World, square: (i32, i32), id: u32, intensity: i32) -> Entity {
    let mut entry = FieldEntry::new(intensity, 100, true, false);
    entry.set_intensity(intensity);
    world
        .spawn((
            Position {
                x: square.0,
                y: square.1,
            },
            Field {
                id: FieldId(id),
                entry,
                priority: 0,
            },
        ))
        .id()
}

#[test]
fn a_field_with_no_processor_data_just_ages() {
    let mut world = World::new();
    let entity = field_at(&mut world, (5, 5), 1, 2);
    let mut rolls = Rolls::new(1);
    let summary = process_field_turn(
        &mut world,
        0,
        0,
        0,
        |_| FieldTurnData::default(),
        |_x, _y| CellFacts::open(),
        &mut rolls,
    );
    assert_eq!(summary.processed, 1);
    assert!(world.get::<Field>(entity).is_some(), "still there");
}

#[test]
fn the_walk_is_in_square_order_so_two_runs_agree() {
    let mut world = World::new();
    field_at(&mut world, (9, 3), 1, 2);
    field_at(&mut world, (2, 7), 1, 2);
    field_at(&mut world, (2, 3), 1, 2);
    let mut rolls = Rolls::new(1);
    let mut control = Rolls::new(1);
    let summary = process_field_turn(
        &mut world,
        0,
        0,
        0,
        |_| FieldTurnData::default(),
        |_x, _y| CellFacts::open(),
        &mut rolls,
    );
    assert_eq!(summary.processed, 3);
    assert_eq!(
        rolls.rng(1, 1000),
        control.rng(1, 1000),
        "decaying ordinary fields draws nothing"
    );
}

#[test]
fn a_spreading_gas_is_attempted_once_per_turn() {
    let mut spread = 0;
    for seed in 1..=60 {
        let mut world = World::new();
        field_at(&mut world, (5, 5), 1, 3);
        let mut rolls = Rolls::new(seed);
        let summary = process_field_turn(
            &mut world,
            0,
            0,
            0,
            |_| FieldTurnData {
                percent_spread: 100,
                ..FieldTurnData::default()
            },
            |_x, _y| CellFacts::open(),
            &mut rolls,
        );
        spread += summary.spread_attempts;
    }
    assert_eq!(spread, 60, "one attempt per turn, not one per neighbour");
}

#[test]
fn a_gas_that_aged_away_is_not_attempted() {
    let mut world = World::new();
    let entity = field_at(&mut world, (5, 5), 1, 1);
    if let Some(mut field) = world.get_mut::<Field>(entity) {
        field.entry.set_age(10_000_000);
    }
    let mut rolls = Rolls::new(1);
    let summary = process_field_turn(
        &mut world,
        10_000_000,
        0,
        0,
        |_| FieldTurnData {
            percent_spread: 100,
            ..FieldTurnData::default()
        },
        |_x, _y| CellFacts::open(),
        &mut rolls,
    );
    assert_eq!(summary.spread_attempts, 0, "gone before spreading");
}

#[test]
fn the_map_decides_what_water_and_sky_do_to_a_field() {
    // The same field, the same prototype data, two different squares: under water it ages, on dry
    // land it does not. The prototype only says how much faster; the terrain says whether.
    let run = |facts: CellFacts| {
        let mut world = World::new();
        let entity = field_at(&mut world, (5, 5), 1, 2);
        let mut rolls = Rolls::new(1);
        process_field_turn(
            &mut world,
            0,
            0,
            0,
            |_| FieldTurnData {
                underwater_age_speedup: 40,
                ..FieldTurnData::default()
            },
            |_x, _y| facts,
            &mut rolls,
        );
        world
            .get::<Field>(entity)
            .map_or(0, |field| field.entry.age())
    };
    assert_eq!(
        run(CellFacts {
            can_enter: true,
            swimmable: true,
            outdoors: false
        }),
        41,
        "one turn of decay plus the forty it ages under water"
    );
    assert_eq!(
        run(CellFacts::open()),
        1,
        "on dry land only the turn of decay"
    );
}
