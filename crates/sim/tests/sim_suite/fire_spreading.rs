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
use cdda_sim::{FIRE, Field, Position, process_fires};

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
            Position { x, y },
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
    for seed in 0..40 {
        let mut world = World::new();
        fire(&mut world, 5, 5, 3, -10_000);
        let mut rolls = Rolls::new(seed);
        process_fires(
            &mut world,
            |_| 0,
            |_| true,
            |_| 0,
            |_| true,
            |_| ground(),
            &mut rolls,
        );
        let mut query = world.query::<&Field>();
        let fires = query.iter(&world).filter(|field| field.id == FIRE).count();
        if fires > 1 {
            let newborn = query
                .iter(&world)
                .find(|field| field.id == FIRE && field.entry.intensity() == 1)
                .expect("the new fire");
            assert_eq!(newborn.entry.age(), -120, "two minutes of fuel, in turns");
            return;
        }
    }
    panic!("the fire never spread over forty seeds");
}

#[test]
fn a_contained_fire_leaves_no_new_fire() {
    let mut world = World::new();
    fire(&mut world, 5, 5, 3, -10_000);
    for seed in 0..20 {
        let mut rolls = Rolls::new(seed);
        process_fires(
            &mut world,
            |_| 0,
            |_| false,
            |_| 0,
            |_| false,
            |_| ground(),
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
    for seed in 0..40 {
        let mut world = World::new();
        let entity = fire(&mut world, 5, 5, 3, -10_000);
        let mut rolls = Rolls::new(seed);
        process_fires(
            &mut world,
            |_| 0,
            |_| true,
            |_| 0,
            |_| true,
            |_| ground(),
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
    for seed in 0..60 {
        let mut world = World::new();
        fire(&mut world, 5, 5, 3, -10_000);
        let mut rolls = Rolls::new(seed);
        process_fires(
            &mut world,
            |_| 0,
            |_| true,
            |_| 0,
            |_| true,
            |_| ground(),
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
