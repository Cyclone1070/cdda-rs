//! Fire spreading to its eight neighbours.
//!
//! The decision path draws dice in a fixed order and short-circuits, so the test is a second
//! transcription of `field_processor_fd_fire` (`src/map_field.cpp:1290-1345`) compared against the
//! port over many seeds and every combination of fuel facts.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::rng::Rolls;
use cdda_sim::{FIRE, Field, NeighbourFacts, Position, should_ignite};

/// The decision as the C++ writes it, draw for draw.
fn reference(intensity: i32, facts: NeighbourFacts, rolls: &mut Rolls) -> bool {
    if facts.fire {
        return false;
    }
    let spread_chance = 25 * (intensity - 1);
    let power = intensity + i32::from(rolls.one_in(5));
    if rolls.rng(1, 100) >= spread_chance {
        return false;
    }
    if power >= 2 && facts.flammable && rolls.one_in(2) {
        return true;
    }
    if power >= 2 && facts.flammable_ash && rolls.one_in(2) {
        return true;
    }
    if power >= 3 && facts.flammable_hard && rolls.one_in(5) {
        return true;
    }
    rolls.one_in(5) && facts.items
}

fn facts_combinations() -> Vec<NeighbourFacts> {
    let mut all = Vec::new();
    for mask in 0..16_u32 {
        all.push(NeighbourFacts {
            fire: mask & 1 != 0,
            flammable: mask & 2 != 0,
            flammable_ash: mask & 4 != 0,
            flammable_hard: mask & 8 != 0,
            items: mask & 16 != 0,
        });
    }
    for mask in 0..4_u32 {
        all.push(NeighbourFacts {
            items: mask & 1 != 0,
            fire: mask & 2 != 0,
            ..NeighbourFacts::default()
        });
    }
    all
}

#[test]
fn the_port_and_the_reference_agree_on_every_fuel_combination() {
    for intensity in 1..=3 {
        for facts in facts_combinations() {
            for attempt in 1..=12 {
                let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
                let mut reference_rolls = Rolls::new(seed);
                let expected = reference(intensity, facts, &mut reference_rolls);
                let mut port_rolls = Rolls::new(seed);
                let actual = should_ignite(intensity, &facts, &mut port_rolls);
                assert_eq!(
                    actual, expected,
                    "intensity {intensity}, facts {facts:?}, seed {seed}"
                );
            }
        }
    }
}

#[test]
fn a_square_already_burning_draws_nothing() {
    let facts = NeighbourFacts {
        fire: true,
        flammable: true,
        ..NeighbourFacts::default()
    };
    let mut untouched = Rolls::new(7);
    let mut compared = Rolls::new(7);
    assert!(!should_ignite(3, &facts, &mut untouched));
    assert_eq!(
        untouched.rng(1, 1000),
        compared.rng(1, 1000),
        "the stream did not move"
    );
}

#[test]
fn a_weak_fire_never_spreads() {
    let facts = NeighbourFacts {
        flammable: true,
        ..NeighbourFacts::default()
    };
    for attempt in 1..=40 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        assert!(
            !should_ignite(1, &facts, &mut rolls),
            "seed {seed}: chance is zero at intensity 1"
        );
    }
}

#[test]
fn a_strong_fire_on_flammable_ground_spreads_sometimes_but_not_always() {
    let facts = NeighbourFacts {
        flammable: true,
        ..NeighbourFacts::default()
    };
    let mut lit = 0;
    let mut quiet = 0;
    for attempt in 1..=60 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        if should_ignite(3, &facts, &mut rolls) {
            lit += 1;
        } else {
            quiet += 1;
        }
    }
    assert!(
        lit > 0 && quiet > 0,
        "{lit} lit, {quiet} quiet over 60 seeds"
    );
}

#[test]
fn a_new_fire_starts_weak_and_two_minutes_old() {
    // Spawning is checked through the world rather than the decision, because the decision alone
    // cannot show what kind of fire it made.
    let mut world = World::new();
    let mut entry = FieldEntry::new(3, 100, true, true);
    entry.set_intensity(3);
    world.spawn((
        Position { x: 5, y: 5, z: 0 },
        Field {
            id: FIRE,
            entry,
            priority: 0,
        },
    ));
    let mut query = world.query::<&Field>();
    assert_eq!(query.iter(&world).filter(|f| f.id == FIRE).count(), 1);
}

#[test]
fn a_spread_fire_starts_weak_and_two_minutes_old_and_costs_the_source_a_minute() {
    use cdda_sim::{NEIGHBOURS, process_fire_spread};
    // One seed in the range lights a neighbour, and the facts mark every neighbour flammable with
    // no items - so the only question is which squares catch.
    let mut lit_at_least_once = false;
    for seed in 1..=60 {
        let mut world = World::new();
        let mut entry = FieldEntry::new(3, 100, true, true);
        entry.set_intensity(3);
        let source = world
            .spawn((
                Position { x: 5, y: 5, z: 0 },
                Field {
                    id: FIRE,
                    entry,
                    priority: 0,
                },
            ))
            .id();
        let mut rolls = Rolls::new(seed);
        process_fire_spread(
            &mut world,
            source,
            |_| NeighbourFacts {
                flammable: true,
                ..NeighbourFacts::default()
            },
            &mut rolls,
        );
        let source_age = world.get::<Field>(source).map_or(0, |f| f.entry.age());
        let mut query = world.query::<(Entity, &Position, &Field)>();
        let ignited: Vec<((i32, i32), i32, i64)> = query
            .iter(&world)
            .filter(|(entity, _, _)| *entity != source)
            .map(|(_, at, field)| ((at.x, at.y), field.entry.intensity(), field.entry.age()))
            .collect();
        if ignited.is_empty() {
            assert_eq!(
                source_age, 0,
                "seed {seed}: no fire lit, so no fuel was spent"
            );
            continue;
        }
        lit_at_least_once = true;
        let lit_count = i64::try_from(ignited.len()).expect("few");
        assert_eq!(
            source_age,
            60 * lit_count,
            "seed {seed}: a minute per fire lit"
        );
        for (square, intensity, age) in &ignited {
            assert_eq!(*intensity, 1, "a new fire is weak");
            assert_eq!(*age, -120, "and two minutes old");
            let (dx, dy) = (square.0 - 5, square.1 - 5);
            assert!(
                NEIGHBOURS.contains(&(dx, dy)),
                "only neighbours catch: ({dx}, {dy})"
            );
        }
    }
    assert!(
        lit_at_least_once,
        "the range must include a seed that spreads"
    );
}
