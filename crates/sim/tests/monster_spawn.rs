//! The gate in front of a spawning field.
//!
//! Transcribed from `field_processor_monster_spawn` (`src/map_field.cpp:763-787`): positive chance
//! and count, then one `one_in(chance)`.

#![allow(clippy::panic, clippy::expect_used)]

use cdda_core_types::rng::Rolls;
use cdda_sim::monster_spawn_attempts;

#[test]
fn a_field_with_no_count_or_no_chance_asks_for_nothing_and_draws_nothing() {
    for (chance, count) in [(0, 3), (15, 0), (0, 0), (-5, 2)] {
        let mut rolls = Rolls::new(1);
        let mut control = Rolls::new(1);
        assert_eq!(monster_spawn_attempts(chance, count, &mut rolls), 0);
        assert_eq!(
            rolls.rng(1, 1000),
            control.rng(1, 1000),
            "chance {chance}, count {count}: no dice"
        );
    }
}

#[test]
fn when_the_roll_fires_the_whole_count_is_asked_for() {
    let mut fired = 0;
    let mut quiet = 0;
    for seed in 1..=400 {
        let mut rolls = Rolls::new(seed);
        let attempts = monster_spawn_attempts(15, 1, &mut rolls);
        if attempts == 1 {
            fired += 1;
        } else {
            quiet += 1;
        }
    }
    assert!(
        fired > 0 && quiet > 0,
        "{fired} fired, {quiet} quiet over 400 seeds"
    );
    // Fifteen in a hundred is about sixty in four hundred; allow a wide band rather than pinning a
    // distribution this test cannot prove.
    assert!(
        (20..=120).contains(&fired),
        "roughly fifteen percent: {fired}"
    );
}

#[test]
fn the_count_comes_back_whole_rather_than_one_at_a_time() {
    let mut world_free_total = 0;
    for seed in 1..=400 {
        let mut rolls = Rolls::new(seed);
        world_free_total += monster_spawn_attempts(15, 3, &mut rolls);
    }
    let mut singles = 0;
    for seed in 1..=400 {
        let mut rolls = Rolls::new(seed);
        if monster_spawn_attempts(15, 1, &mut rolls) == 1 {
            singles += 1;
        }
    }
    assert_eq!(
        world_free_total,
        singles * 3,
        "three at a time, never one three times"
    );
}
