//! A whole turn of fire, wired in the reference's order.
//!
//! The individual steps are gated by their own tests; what this one pins is the **order** and the
//! wiring, because the same steps in a different order draw different dice and leave a different
//! world. The comparison is against a manual composition of the same steps - so it proves the
//! sequence, not the steps.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::rng::Rolls;
use cdda_sim::{
    FIRE, Field, FireContext, NeighbourFacts, Position, TerrainFuel, burn_terrain, process_fire,
    process_fire_burnout, process_fire_residue, process_fire_spread, process_flashpoint,
};

fn world_with_fire(intensity: i32, age: i64) -> (World, Entity) {
    let mut world = World::new();
    let mut entry = FieldEntry::new(3, 100, true, true);
    entry.set_intensity(intensity);
    entry.set_age(age);
    let entity = world
        .spawn((
            Position { x: 5, y: 5 },
            Field {
                id: FIRE,
                entry,
                priority: 0,
            },
        ))
        .id();
    (world, entity)
}

fn snapshot(world: &mut World) -> Vec<(u32, i32, i64, (i32, i32))> {
    let mut query = world.query::<(&Field, &Position)>();
    let mut all: Vec<(u32, i32, i64, (i32, i32))> = query
        .iter(world)
        .map(|(field, at)| {
            (
                field.id.0,
                field.entry.intensity(),
                field.entry.age(),
                (at.x, at.y),
            )
        })
        .collect();
    all.sort_unstable();
    all
}

fn facts(_square: (i32, i32)) -> NeighbourFacts {
    NeighbourFacts {
        flammable: true,
        ..NeighbourFacts::default()
    }
}

#[test]
fn a_turn_of_fire_matches_the_steps_run_in_order() {
    let fuel = TerrainFuel {
        flammable: true,
        ..TerrainFuel::default()
    };
    for seed in 1..=20 {
        for (intensity, age) in [(1, -600), (2, -600), (3, -600)] {
            let (mut wired, wired_fire) = world_with_fire(intensity, age);
            let context = FireContext {
                fuel,
                neighbour: &facts,
                windpower: 0,
                consumed: 0,
                smoke_suppressed: true,
            };
            let mut rolls = Rolls::new(seed);
            process_fire(&mut wired, wired_fire, &context, &mut rolls);

            // The same steps, by hand, in the documented order.
            let (mut manual, manual_fire) = world_with_fire(intensity, age);
            let mut rolls = Rolls::new(seed);
            let burn = burn_terrain(fuel, intensity, 0, &mut rolls);
            let smoke = burn.map_or(0, |burn| burn.smoke);
            if let Some(burn) = burn
                && burn.time_added != 0
            {
                if let Some(mut field) = manual.get_mut::<Field>(manual_fire) {
                    let age = field.entry.age();
                    field.entry.set_age(age - burn.time_added);
                }
            } else {
                process_fire_burnout(&mut manual, manual_fire, burn.is_some());
            }
            let adjacent = if intensity > 1 {
                process_flashpoint(&mut manual, manual_fire, &mut rolls)
            } else {
                0
            };
            process_fire_spread(&mut manual, manual_fire, facts, &mut rolls);
            process_fire_residue(
                &mut manual,
                manual_fire,
                smoke,
                0,
                true,
                adjacent,
                &mut rolls,
            );

            assert_eq!(
                snapshot(&mut wired),
                snapshot(&mut manual),
                "seed {seed}, intensity {intensity}: the wiring must match the sequence"
            );
        }
    }
}

#[test]
fn a_fire_on_burning_ground_gains_life_rather_than_losing_it() {
    let fuel = TerrainFuel {
        flammable: true,
        ..TerrainFuel::default()
    };
    let (mut world, fire) = world_with_fire(2, -600);
    let context = FireContext {
        fuel,
        neighbour: &facts,
        windpower: 0,
        consumed: 0,
        smoke_suppressed: true,
    };
    let mut rolls = Rolls::new(1);
    process_fire(&mut world, fire, &context, &mut rolls);
    let age = world
        .get::<Field>(fire)
        .map_or(0, |field| field.entry.age());
    assert!(
        age < -600,
        "plain ground gives 5 - 2 = 3 turns of life: {age}"
    );
}

#[test]
fn a_fire_on_bare_ground_starves() {
    let fuel = TerrainFuel::default();
    let (mut world, fire) = world_with_fire(2, -600);
    let context = FireContext {
        fuel,
        neighbour: &facts,
        windpower: 0,
        consumed: 0,
        smoke_suppressed: true,
    };
    let mut rolls = Rolls::new(1);
    process_fire(&mut world, fire, &context, &mut rolls);
    let age = world
        .get::<Field>(fire)
        .map_or(0, |field| field.entry.age());
    assert_eq!(age, -580, "bare ground costs twenty seconds at intensity 2");
}
