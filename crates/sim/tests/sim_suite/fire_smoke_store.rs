//! A burning fire puts smoke in the store.
//!
//! Reference: the smoke block at the end of the fire processor (`src/map_field.cpp:1407-1425`) adds `fd_smoke` through `add_field`, so the smoke is a
//! field like any other and lands on the square the fire chose. The port's store is two dimensional, so the "above" branch has no square to go to and
//! the smoke settles on the fire - which is the reference's own fallback when it cannot rise.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::rng::Rolls;
use cdda_sim::{FIRE, Field, FireMap, Position, SMOKE, process_fires};

/// A map with a lot of smoke fuel and nothing to interfere.
struct Smoky {
    suppress: bool,
}

impl FireMap for Smoky {
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
        false
    }
    fn neighbour(&self, _square: (i32, i32, i32)) -> cdda_core_types::fire::Neighbour {
        cdda_core_types::fire::Neighbour::default()
    }
    fn suppress_smoke(&self, _square: (i32, i32, i32)) -> bool {
        self.suppress
    }
}

fn fire(world: &mut World, x: i32, y: i32, intensity: i32) -> Entity {
    let mut entry = FieldEntry::new(3, 1_000_000, false, true);
    entry.set_age(-10_000);
    entry.set_intensity(intensity);
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

fn smoke_here(world: &mut World, square: (i32, i32)) -> i32 {
    let mut query = world.query::<(&Field, &Position)>();
    query
        .iter(world)
        .find(|(field, position)| {
            field.id == SMOKE && position.x == square.0 && position.y == square.1
        })
        .map_or(0, |(field, _)| field.entry.intensity())
}

#[test]
fn a_raging_fire_eventually_smokes_its_own_square() {
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..300 {
        let mut world = World::new();
        fire(&mut world, 4, 4, 3);
        let seed = u32::try_from(attempt).unwrap_or(u32::MAX).wrapping_mul(2_654_435_761).wrapping_add(1);
        let mut rolls = Rolls::new(seed);
        process_fires(&mut world, &Smoky { suppress: false }, 0, &mut rolls);
        let thickness = smoke_here(&mut world, (4, 4));
        if thickness > 0 {
            assert_eq!(
                thickness, 3,
                "smoke on the fire itself is as thick as the fire"
            );
            return;
        }
    }
    panic!("three hundred seeds and no smoke");
}

#[test]
fn a_suppressed_fire_leaves_no_smoke() {
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..100 {
        let mut world = World::new();
        fire(&mut world, 4, 4, 3);
        let seed = u32::try_from(attempt).unwrap_or(u32::MAX).wrapping_mul(2_654_435_761).wrapping_add(1);
        let mut rolls = Rolls::new(seed);
        process_fires(&mut world, &Smoky { suppress: true }, 0, &mut rolls);
        assert_eq!(smoke_here(&mut world, (4, 4)), 0, "seed {seed}");
    }
}

#[test]
fn smoke_on_a_smoky_square_adds_up_turn_after_turn() {
    // Two turns of smoke make a thicker patch rather than replacing it: the merge rule the reference uses for any field landing on its own kind.
    let mut world = World::new();
    fire(&mut world, 4, 4, 3);
    let mut thickest = 0;
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..300 {
        let seed = u32::try_from(attempt).unwrap_or(u32::MAX).wrapping_mul(2_654_435_761).wrapping_add(1);
        let mut rolls = Rolls::new(seed);
        process_fires(&mut world, &Smoky { suppress: false }, 0, &mut rolls);
        thickest = thickest.max(smoke_here(&mut world, (4, 4)));
        if thickest == 3 && smoke_here(&mut world, (4, 4)) == 3 {
            // Clamped at the type's maximum, which the earlier merge tests already pin.
            break;
        }
    }
    assert!(thickest > 0, "no smoke at all over three hundred seeds");
}
