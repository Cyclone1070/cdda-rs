//! A fire that spreads puts a new fire in the store.
//!
//! Reference: the spread loop ends with `here.add_field( dst_p, fd_fire, 1, 2_minutes, false )` and charges the spreading fire a minute of its
//! own life (`src/map_field.cpp:1338-1345`). The new fire is **weak and fuelled**: intensity one with two minutes in the tank, so it does not
//! immediately jump again.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::fire::Neighbour;
use cdda_core_types::rng::Rolls;
use cdda_sim::{FIRE, Field, FireMap, Position, process_fires};

/// A map that answers every question the same way everywhere, so a test varies one answer at a time.
#[derive(Default)]
struct FlatMap {
    adjacent_fires: i32,
    can_burn: bool,
    ground: cdda_core_types::fire::Ground,
    can_spread: bool,
    neighbour: Neighbour,
    suppress_smoke: bool,
}

impl FireMap for FlatMap {
    fn adjacent_fires(&self, _square: (i32, i32, i32)) -> i32 {
        self.adjacent_fires
    }
    fn can_burn(&self, _square: (i32, i32, i32)) -> bool {
        self.can_burn
    }
    fn ground(&self, _square: (i32, i32, i32)) -> cdda_core_types::fire::Ground {
        self.ground
    }
    fn can_spread(&self, _square: (i32, i32, i32)) -> bool {
        self.can_spread
    }
    fn neighbour(&self, _square: (i32, i32, i32)) -> Neighbour {
        self.neighbour
    }
    fn suppress_smoke(&self, _square: (i32, i32, i32)) -> bool {
        self.suppress_smoke
    }
}

fn fire(world: &mut World, x: i32, y: i32, intensity: i32, age: i64) -> Entity {
    let mut entry = FieldEntry::new(3, 1_000_000, false, true);
    entry.set_age(age);
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

fn ground() -> Neighbour {
    Neighbour {
        flammable: true,
        ..Neighbour::default()
    }
}

#[test]
fn a_spreading_fire_leaves_a_weak_fuelled_fire_behind() {
    // Intensity three on burnable ground with a generous number of seeds: somewhere in the first few the fire jumps, and what it leaves is
    // intensity one with two minutes of fuel - negative age, which is how the reference writes fuel.
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..40 {
        let mut world = World::new();
        fire(&mut world, 5, 5, 3, -10_000);
        let seed = u32::try_from(attempt)
            .unwrap_or(u32::MAX)
            .wrapping_mul(2_654_435_761)
            .wrapping_add(1);
        let mut rolls = Rolls::new(seed);
        process_fires(
            &mut world,
            &FlatMap {
                adjacent_fires: 0,
                can_burn: true,
                ground: cdda_core_types::fire::Ground::Inert,
                can_spread: true,
                neighbour: ground(),
                ..FlatMap::default()
            },
            0,
            &mut rolls,
        );
        let mut query = world.query::<&Field>();
        let fires = query.iter(&world).filter(|field| field.id == FIRE).count();
        if fires > 1 {
            let newborn = query
                .iter(&world)
                .find(|field| field.id == FIRE && field.entry.intensity() == 1)
                .expect("the new fire");
            // The reference adds the new fire with `add_field( dst_p, fd_fire, 1, 2_minutes )`, and a field's age is
            // **time burned**, not fuel: "Lower age is a longer lasting fire" (`map_field.cpp:1122`). So the newborn
            // is older and weaker than a fresh fire - it will not grow until it eats - and my first version had it at
            // -120, which would have made it a long-lived fire instead of a short-lived one.
            assert_eq!(
                newborn.entry.age(),
                120,
                "two minutes of burning already behind it"
            );
            return;
        }
    }
    panic!("the fire never spread over forty seeds");
}

#[test]
fn a_contained_fire_leaves_no_new_fire() {
    let mut world = World::new();
    fire(&mut world, 5, 5, 3, -10_000);
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..20 {
        let seed = u32::try_from(attempt)
            .unwrap_or(u32::MAX)
            .wrapping_mul(2_654_435_761)
            .wrapping_add(1);
        let mut rolls = Rolls::new(seed);
        process_fires(
            &mut world,
            &FlatMap {
                adjacent_fires: 0,
                can_burn: false,
                ground: cdda_core_types::fire::Ground::Inert,
                can_spread: false,
                neighbour: ground(),
                ..FlatMap::default()
            },
            0,
            &mut rolls,
        );
        let mut query = world.query::<&Field>();
        assert_eq!(
            query.iter(&world).filter(|field| field.id == FIRE).count(),
            1,
            "seed {seed}"
        );
    }
}

#[test]
fn the_spreading_fire_spends_a_minute_of_its_own_life() {
    // `cur.set_field_age( cur.get_field_age() + 1_minutes )` on the fire that spread - sixty turns closer to burning out.
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..40 {
        let mut world = World::new();
        let entity = fire(&mut world, 5, 5, 3, -10_000);
        let seed = u32::try_from(attempt)
            .unwrap_or(u32::MAX)
            .wrapping_mul(2_654_435_761)
            .wrapping_add(1);
        let mut rolls = Rolls::new(seed);
        process_fires(
            &mut world,
            &FlatMap {
                adjacent_fires: 0,
                can_burn: true,
                ground: cdda_core_types::fire::Ground::Inert,
                can_spread: true,
                neighbour: ground(),
                ..FlatMap::default()
            },
            0,
            &mut rolls,
        );
        let mut query = world.query::<&Field>();
        let count = query.iter(&world).filter(|field| field.id == FIRE).count();
        if count > 1 {
            let parent = world.get::<Field>(entity).expect("the original fire");
            // Thirty turns of the ten-second drain for standing on burnable ground, then sixty for spreading: the drain happens first, which
            // is the same order that caught me out last round.
            assert_eq!(
                parent.entry.age(),
                -10_000 + 30 + 60,
                "a minute paid, after the drain"
            );
            return;
        }
    }
    panic!("the fire never spread over forty seeds");
}

#[test]
fn each_new_fire_is_put_on_a_different_square() {
    // A neighbour that catches gets one fire, not one per roll: the store is keyed by entity but the world is keyed by square, and two fires on
    // one square would double its damage and its light.
    // Spread seeds, not counted up: `uniform_int` maps a small engine value to a small answer and a small seed's first draw is small,
    // so `0, 1, 2, ...` samples one end of the distribution rather than the chance under test.
    for attempt in 0..60 {
        let mut world = World::new();
        fire(&mut world, 5, 5, 3, -10_000);
        let seed = u32::try_from(attempt)
            .unwrap_or(u32::MAX)
            .wrapping_mul(2_654_435_761)
            .wrapping_add(1);
        let mut rolls = Rolls::new(seed);
        process_fires(
            &mut world,
            &FlatMap {
                adjacent_fires: 0,
                can_burn: true,
                ground: cdda_core_types::fire::Ground::Inert,
                can_spread: true,
                neighbour: ground(),
                ..FlatMap::default()
            },
            0,
            &mut rolls,
        );
        let mut query = world.query::<(&Field, &Position)>();
        let mut squares: Vec<(i32, i32)> = query
            .iter(&world)
            .filter(|(field, _)| field.id == FIRE)
            .map(|(_, position)| (position.x, position.y))
            .collect();
        let before = squares.len();
        squares.sort_unstable();
        squares.dedup();
        assert_eq!(
            squares.len(),
            before,
            "seed {seed} put two fires on one square"
        );
    }
}
