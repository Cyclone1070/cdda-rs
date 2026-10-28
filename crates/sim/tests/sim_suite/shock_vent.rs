//! The shock vent, checked against a second transcription of the reference.
//!
//! `field_processor_fd_shock_vent` (`src/map_field.cpp:838-876`) draws a lot of dice in a specific
//! order, so the test that matters is not "some bolts appeared": it is that the port and an
//! independent transcription of the same C++ draw the same numbers in the same order and light the
//! same squares. The shadow below is written from the reference, not from the port.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::rng::Rolls;
use cdda_sim::{ELECTRICITY, Field, Position, SHOCK_VENT, process_shock_vent};
use std::collections::BTreeMap;

/// The reference's algorithm, transcribed straight from the C++, keeping the same draw order.
fn reference(seed: u32, start: (i32, i32), intensity: i32) -> (i32, BTreeMap<(i32, i32), i32>) {
    let mut rolls = Rolls::new(seed);
    let mut vent = intensity;
    let mut squares: BTreeMap<(i32, i32), i32> = BTreeMap::new();
    if vent > 1 {
        if rolls.one_in(5) {
            vent -= 1;
        }
        return (vent, squares);
    }
    vent = 3;
    let bolts = rolls.rng(3, 6);
    for _ in 0..bolts {
        let (mut dx, mut dy) = (0, 0);
        while dx == 0 && dy == 0 {
            dx = rolls.rng(-1, 1);
            dy = rolls.rng(-1, 1);
        }
        let length = rolls.rng(4, 12);
        let (mut x, mut y) = start;
        for _ in 0..length {
            x += dx;
            y += dy;
            let strength = rolls.rng(2, 3);
            let slot = squares.entry((x, y)).or_insert(0);
            *slot = (*slot).max(strength);
            if rolls.one_in(4) {
                dx = if dx == 0 { rolls.rng(0, 1) * 2 - 1 } else { 0 };
            }
            if rolls.one_in(4) {
                dy = if dy == 0 { rolls.rng(0, 1) * 2 - 1 } else { 0 };
            }
        }
    }
    (vent, squares)
}

fn run(seed: u32, intensity: i32) -> (i32, BTreeMap<(i32, i32), i32>) {
    let mut world = World::new();
    let mut entry = FieldEntry::new(3, 100, true, false);
    entry.set_intensity(intensity);
    let vent = world
        .spawn((
            Position { x: 10, y: 10, z: 0 },
            Field {
                id: SHOCK_VENT,
                entry,
                priority: 0,
            },
        ))
        .id();
    let mut rolls = Rolls::new(seed);
    process_shock_vent(&mut world, vent, &mut rolls);
    let vent_intensity = world
        .get::<Field>(vent)
        .map_or(0, |field| field.entry.intensity());
    let mut squares = BTreeMap::new();
    let mut query = world.query::<(&Position, &Field)>();
    for (at, field) in query.iter(&world) {
        if field.id == ELECTRICITY {
            squares.insert((at.x, at.y), field.entry.intensity());
        }
    }
    (vent_intensity, squares)
}

#[test]
fn twenty_seeds_light_exactly_what_the_reference_lights() {
    for seed in 1..=20 {
        let expected = reference(seed, (10, 10), 1);
        let actual = run(seed, 1);
        assert_eq!(actual.0, expected.0, "seed {seed}: vent intensity");
        assert_eq!(
            actual.1, expected.1,
            "seed {seed}: lit squares and their strength"
        );
    }
}

#[test]
fn a_strong_vent_thins_one_time_in_five_and_lays_no_bolts() {
    let mut thinned = 0;
    let mut stayed = 0;
    for attempt in 1..=40_u32 {
        let seed = crate::spread_seed(attempt);
        let expected = reference(seed, (10, 10), 2);
        let actual = run(seed, 2);
        assert_eq!(actual, expected, "seed {seed}");
        assert!(actual.1.is_empty(), "a strong vent draws no bolts");
        match actual.0 {
            1 => thinned += 1,
            2 => stayed += 1,
            other => panic!("unexpected intensity {other}"),
        }
    }
    assert!(
        thinned > 0 && stayed > 0,
        "both outcomes over 40 seeds: {thinned} thinned, {stayed} stayed"
    );
}

#[test]
fn a_spent_vent_always_refills_to_three() {
    for seed in 1..=10 {
        assert_eq!(run(seed, 1).0, 3, "seed {seed}");
    }
}
