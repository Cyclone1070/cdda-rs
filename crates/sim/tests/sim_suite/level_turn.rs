//! A whole environment turn: fires, then the gases they leave, then decay.
//!
//! Reference: `map::process_fields` (`src/map_field.cpp:164-200`) walks the submaps once per turn, and inside it each field's processor runs and the field
//! then decays. The port splits that into three systems for clarity - `process_fires`, the gas systems, and `process_field_turns` for the decay - and this
//! is the function that runs them in the reference's order, so a test can burn a room for twenty turns instead of calling three systems by hand.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::fire::Neighbour;
use cdda_core_types::rng::Rolls;
use cdda_sim::{
    FIRE, Field, FireMap, HOT_AIR_1, HOT_AIR_2, HOT_AIR_3, HOT_AIR_4, Position, SMOKE,
    run_environment_turn,
};

/// Ground that burns hard: `FLAMMABLE_HARD` for ever, no items, no wind.
struct HardFloor;

impl FireMap for HardFloor {
    fn adjacent_fires(&self, _square: (i32, i32, i32)) -> i32 {
        0
    }
    fn can_burn(&self, _square: (i32, i32, i32)) -> bool {
        true
    }
    fn ground(&self, _square: (i32, i32, i32)) -> cdda_core_types::fire::Ground {
        cdda_core_types::fire::Ground::Inert
    }
    fn can_spread(&self, _square: (i32, i32, i32)) -> bool {
        true
    }
    fn neighbour(&self, _square: (i32, i32, i32)) -> Neighbour {
        Neighbour {
            flammable_hard: true,
            ..Neighbour::default()
        }
    }
    fn suppress_smoke(&self, _square: (i32, i32, i32)) -> bool {
        false
    }
}

fn every_square_takes(_from: (i32, i32, i32), _to: (i32, i32, i32)) -> bool {
    true
}

fn place_fire(world: &mut World, x: i32, y: i32) -> Entity {
    let mut entry = FieldEntry::new(3, 1_000_000, false, true);
    entry.set_age(1);
    entry.set_intensity(3);
    world
        .spawn((
            Field {
                id: FIRE,
                entry,
                priority: 0,
            },
            Position { x, y, z: 0 },
        ))
        .id()
}

/// Every field on the level, sorted, so two runs can be compared without depending on entity ids.
fn snapshot(world: &mut World) -> Vec<(i32, i32, u32, i32, i64)> {
    let mut query = world.query::<(&Field, &Position)>();
    let mut rows: Vec<(i32, i32, u32, i32, i64)> = query
        .iter(world)
        .map(|(field, position)| {
            (
                position.x,
                position.y,
                field.id.0,
                field.entry.intensity(),
                field.entry.age(),
            )
        })
        .collect();
    rows.sort_unstable();
    rows
}

#[test]
fn a_burning_room_fills_with_smoke_and_haze_over_twenty_turns() {
    let mut world = World::new();
    place_fire(&mut world, 10, 10);
    let mut rolls = Rolls::new(4_242);
    for _ in 0..20 {
        run_environment_turn(&mut world, &HardFloor, 0, every_square_takes, &mut rolls);
    }
    let state = snapshot(&mut world);
    let fires = state.iter().filter(|row| row.2 == FIRE.0).count();
    let smoke = state.iter().filter(|row| row.2 == SMOKE.0).count();
    let hot_air = state
        .iter()
        .filter(|row| [HOT_AIR_1.0, HOT_AIR_2.0, HOT_AIR_3.0, HOT_AIR_4.0].contains(&row.2))
        .count();
    assert!(fires >= 1, "the fire is still burning: {state:?}");
    assert!(
        smoke > 0,
        "twenty turns of a raging fire left smoke somewhere"
    );
    assert!(hot_air > 0, "and haze");
}

#[test]
fn the_same_seed_burns_the_same_room_twice() {
    // Determinism is the property every comparison rests on: two worlds with the same seed and the same turn count must be identical, field for field.
    let run = || {
        let mut world = World::new();
        place_fire(&mut world, 10, 10);
        let mut rolls = Rolls::new(777);
        for _ in 0..12 {
            run_environment_turn(&mut world, &HardFloor, 0, every_square_takes, &mut rolls);
        }
        snapshot(&mut world)
    };
    assert_eq!(run(), run(), "two identical runs diverged");
}

#[test]
fn a_different_seed_burns_it_differently() {
    // The counterpart: if the seed made no difference the determinism test above would pass for the wrong reason.
    let run = |seed: u32| {
        let mut world = World::new();
        place_fire(&mut world, 10, 10);
        let mut rolls = Rolls::new(seed);
        for _ in 0..12 {
            run_environment_turn(&mut world, &HardFloor, 0, every_square_takes, &mut rolls);
        }
        snapshot(&mut world)
    };
    assert_ne!(run(1), run(2), "two seeds produced the same room");
}

#[test]
fn an_empty_level_stays_empty() {
    // The turn must not invent fields: with nothing on the level, twenty turns leave nothing.
    let mut world = World::new();
    let mut rolls = Rolls::new(5);
    for _ in 0..20 {
        run_environment_turn(&mut world, &HardFloor, 0, every_square_takes, &mut rolls);
    }
    assert!(snapshot(&mut world).is_empty());
}
