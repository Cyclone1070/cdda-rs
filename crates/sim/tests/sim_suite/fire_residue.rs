//! What a fire leaves behind: smoke and hot air.
//!
//! Transcribed from `field_processor_fd_fire` (`src/map_field.cpp:1400-1425`). The dice order is
//! `rng(0, 100 - windpower)`, then `rng(3, 35)`, then hot air's `rng(0, 4 - adjacent)`, and the
//! first two are skipped entirely when the terrain suppresses smoke.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::rng::Rolls;
use cdda_sim::{FIRE, Field, Position, SMOKE, process_fire_residue};

fn burning(world: &mut World, intensity: i32) -> Entity {
    let mut entry = FieldEntry::new(3, 100, true, true);
    entry.set_intensity(intensity);
    world
        .spawn((
            Position { x: 5, y: 5, z: 0 },
            Field {
                id: FIRE,
                entry,
                priority: 0,
            },
        ))
        .id()
}

fn smoke_strength(world: &mut World) -> Option<i32> {
    let mut query = world.query::<&Field>();
    query
        .iter(world)
        .find(|field| field.id == SMOKE)
        .map(|field| field.entry.intensity())
}

/// The reference's rule, transcribed separately: returns whether smoke is made.
fn reference_smoke(
    intensity: i32,
    smoke: i32,
    windpower: i32,
    suppressed: bool,
    rolls: &mut Rolls,
) -> bool {
    if suppressed {
        return false;
    }
    if rolls.rng(0, 100 - windpower) > smoke {
        return false;
    }
    rolls.rng(3, 35) < intensity * 10
}

#[test]
fn smoke_appears_exactly_when_the_reference_says_it_does() {
    for intensity in 1..=3 {
        for attempt in 1..=40 {
            let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
            let mut expected_rolls = Rolls::new(seed);
            let expected = reference_smoke(intensity, 0, 0, false, &mut expected_rolls);
            let mut world = World::new();
            let source = burning(&mut world, intensity);
            let mut rolls = Rolls::new(seed);
            process_fire_residue(&mut world, source, 0, 0, false, 0, &mut rolls);
            assert_eq!(
                smoke_strength(&mut world).is_some(),
                expected,
                "intensity {intensity}, seed {seed}"
            );
        }
    }
}

#[test]
fn a_suppressing_terrain_skips_the_smoke_dice_entirely() {
    let mut world = World::new();
    let source = burning(&mut world, 3);
    let mut rolls = Rolls::new(1);
    let mut control = Rolls::new(1);
    process_fire_residue(&mut world, source, 0, 0, true, 0, &mut rolls);
    assert_eq!(smoke_strength(&mut world), None, "no smoke");
    // The first draw the call may make is hot air's; the smoke draws must not have happened.
    let first = rolls.rng(0, 4);
    let expected = control.rng(0, 4);
    assert_eq!(first, expected, "the two smoke draws were skipped");
}

#[test]
fn strong_fires_make_smoke_more_often_than_weak_ones() {
    let mut weak = 0;
    let mut strong = 0;
    for attempt in 1..=80 {
        let seed = crate::spread_seed(u32::try_from(attempt).unwrap_or(u32::MAX));
        let mut world = World::new();
        let source = burning(&mut world, 1);
        let mut rolls = Rolls::new(seed);
        // `smoke` of a hundred makes the first gate certain (`rng(0, 100) <= 100`), so what is compared is the second: `rng(3, 35) < intensity * 10`.
        process_fire_residue(&mut world, source, 100, 0, false, 0, &mut rolls);
        if smoke_strength(&mut world).is_some() {
            weak += 1;
        }

        let mut world = World::new();
        let source = burning(&mut world, 3);
        let mut rolls = Rolls::new(seed);
        process_fire_residue(&mut world, source, 100, 0, false, 0, &mut rolls);
        if smoke_strength(&mut world).is_some() {
            strong += 1;
        }
    }
    assert!(
        strong > weak,
        "intensity 3 smokes more often: {strong} against {weak}"
    );
    // With the first gate forced open, the second is `rng(3, 35) < intensity * 10`: a fifth for a weak fire (`< 10`), six sevenths for a strong one (`< 30`). The
    // measured rates over eighty spread seeds are in the assertion, so a change in either constant shows up as a number rather than as a pass.
    assert_eq!(
        weak, 17,
        "a weak fire smokes when the range's low end comes up: {weak} of 80"
    );
    assert!(
        strong > 60,
        "and a strong one most of the time: {strong} of 80"
    );
}

#[test]
fn many_neighbouring_fires_put_out_the_hot_air() {
    // Five or more adjacent fires mean no hot air at all, and no draw either.
    let mut world = World::new();
    let source = burning(&mut world, 3);
    let mut rolls = Rolls::new(1);
    let mut control = Rolls::new(1);
    process_fire_residue(&mut world, source, 0, 0, true, 5, &mut rolls);
    assert_eq!(
        rolls.rng(1, 1000),
        control.rng(1, 1000),
        "nothing was drawn"
    );
}
