//! What the two damaging gases do to a creature.
//!
//! Transcribed from `field_processor_fd_toxic_gas` and `field_processor_fd_nuke_gas`
//! (`src/map_field.cpp:1966-1990`).

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::rng::Rolls;
use cdda_sim::creature::{health_of, moves_left};
use cdda_sim::{
    GasEffect, MonsterKindId, breathe_gas, nuke_gas_effect, spawn_monster, toxic_gas_effect,
};

/// The nuke gas rule as the C++ writes it.
fn reference(intensity: i32, no_breathe: bool, veggy: bool, rolls: &mut Rolls) -> GasEffect {
    if no_breathe {
        return GasEffect {
            damage: 0,
            moves_penalty: 0.0,
        };
    }
    let (mut moves_penalty, mut damage) = match intensity {
        3 => (rolls.rng_float(0.6, 1.2), rolls.rng(30, 50)),
        2 => (rolls.rng_float(0.2, 0.5), rolls.rng(10, 25)),
        _ => (rolls.rng_float(0.0, 0.15), rolls.rng(0, 12)),
    };
    if veggy {
        moves_penalty += rolls.rng_float(f64::from(intensity) * 0.05, f64::from(intensity) * 0.12);
        damage *= intensity;
    }
    GasEffect {
        damage,
        moves_penalty,
    }
}

#[test]
fn toxic_gas_damages_by_intensity_and_draws_nothing() {
    for intensity in 1..=3 {
        let effect = toxic_gas_effect(intensity, false);
        assert_eq!(effect.damage, intensity);
        assert!((effect.moves_penalty - f64::from(intensity)).abs() < 1e-9);
    }
}

#[test]
fn a_creature_that_does_not_breathe_takes_nothing_from_toxic_gas() {
    assert_eq!(
        toxic_gas_effect(3, true),
        GasEffect {
            damage: 0,
            moves_penalty: 0.0
        }
    );
}

#[test]
fn nuke_gas_matches_the_reference_over_every_intensity_and_kind() {
    for intensity in 1..=3 {
        for no_breathe in [false, true] {
            for veggy in [false, true] {
                for attempt in 1..=20 {
                    let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
                    let mut expected_rolls = Rolls::new(seed);
                    let expected = reference(intensity, no_breathe, veggy, &mut expected_rolls);
                    let mut rolls = Rolls::new(seed);
                    let actual = nuke_gas_effect(intensity, no_breathe, veggy, &mut rolls);
                    assert_eq!(
                        actual, expected,
                        "i {intensity}, no_breathe {no_breathe}, veggy {veggy}, seed {seed}"
                    );
                }
            }
        }
    }
}

#[test]
fn plants_take_nuke_gas_worse_than_animals() {
    let mut plant_total = 0;
    let mut animal_total = 0;
    for attempt in 1..=30 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut rolls = Rolls::new(seed);
        plant_total += nuke_gas_effect(2, false, true, &mut rolls).damage;
        let mut rolls = Rolls::new(seed);
        animal_total += nuke_gas_effect(2, false, false, &mut rolls).damage;
    }
    assert!(
        plant_total > animal_total,
        "plants {plant_total} against animals {animal_total}"
    );
}

#[test]
fn breathing_a_gas_hurts_and_slows_the_creature_standing_in_it() {
    let mut world = World::new();
    let creature = spawn_monster(&mut world, (5, 5), MonsterKindId(1), 20);
    let effect = GasEffect {
        damage: 5,
        moves_penalty: 40.0,
    };
    assert!(!breathe_gas(&mut world, creature, &effect), "fifteen left");
    assert_eq!(health_of(&world, creature), Some(15), "fifteen left");
    assert_eq!(moves_left(&world, creature), 60, "moves went down by forty");
}

#[test]
fn a_lethal_gas_breath_takes_the_creature_off_the_map() {
    let mut world = World::new();
    let creature = spawn_monster(&mut world, (5, 5), MonsterKindId(1), 3);
    let effect = GasEffect {
        damage: 9,
        moves_penalty: 10.0,
    };
    assert!(breathe_gas(&mut world, creature, &effect));
    assert!(health_of(&world, creature).is_none(), "and it is gone");
}
