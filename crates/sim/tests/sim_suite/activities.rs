//! Things that take more than one turn.
//!
//! Reference: `player_activity::do_turn` (`player_activity.cpp:276-294`). The rule has two halves that look
//! interchangeable and are not: a **time**-based activity takes a hundred moves a turn whatever the creature's speed,
//! and a **speed**-based one takes everything the creature has. A quick creature therefore finishes a speed-based job
//! sooner and a time-based one no sooner at all - which is the difference between reading a book and digging a hole.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_sim::creature::{Basis, Doing, give_moves, moves_left, set_speed, spawn_creature};
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{Prototypes, singletons};

const FLOOR: u32 = 1;

fn a_world() -> World {
    let mut world = World::new();
    let mut level = Level::new();
    for x in 0..4 {
        level.set_terrain(x, 0, FLOOR, FLOOR);
    }
    cdda_sim::load_level(&mut world, &level);
    let mut terrain = TerrainTable::new();
    terrain.insert(FLOOR, true);
    terrain.insert_move_cost(FLOOR, 2);
    cdda_sim::set_prototypes(
        &mut world,
        Prototypes {
            terrain,
            furniture: TerrainTable::new(),
        },
    );
    singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));
    world
}

fn doing(activity: &str, moves: i32, basis: Basis) -> Doing {
    Doing {
        activity: activity.to_owned(),
        moves_left: moves,
        basis,
    }
}

#[test]
fn a_time_based_job_takes_the_same_three_turns_for_a_quick_creature_and_a_slow_one() {
    let turns = |speed: i32| {
        let mut world = a_world();
        // Two creatures so the turn loop has something to run for; the busy one is the one being timed.
        let busy = spawn_creature(&mut world, "the survivor", (0, 0, 0));
        set_speed(&mut world, busy, speed, 0);
        world
            .entity_mut(busy)
            .insert(doing("reading", 250, Basis::Time));
        let mut turns = 0;
        for _ in 0..10 {
            cdda_sim::turn::run_turn(&mut world);
            turns += 1;
            if world.get::<Doing>(busy).is_none() {
                break;
            }
        }
        turns
    };

    assert_eq!(turns(100), 3, "two hundred and fifty moves at a hundred a turn");
    assert_eq!(
        turns(200),
        3,
        "and the same for a creature twice as quick: a time-based job is not quicker for being quick"
    );
}

#[test]
fn a_speed_based_job_is_quicker_for_a_quicker_creature() {
    let turns = |speed: i32| {
        let mut world = a_world();
        let busy = spawn_creature(&mut world, "the survivor", (0, 0, 0));
        set_speed(&mut world, busy, speed, 0);
        world
            .entity_mut(busy)
            .insert(doing("digging", 400, Basis::Speed));
        let mut turns = 0;
        for _ in 0..20 {
            cdda_sim::turn::run_turn(&mut world);
            turns += 1;
            if world.get::<Doing>(busy).is_none() {
                break;
            }
        }
        turns
    };

    assert_eq!(turns(100), 4, "four hundred moves at a hundred a turn");
    assert_eq!(turns(200), 2, "and two turns for a creature that has twice the moves");
}

#[test]
fn a_busy_creature_does_not_walk() {
    let mut world = a_world();
    let busy = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    give_moves(&mut world, busy, 100);
    world
        .entity_mut(busy)
        .insert(doing("reading", 1_000, Basis::Time));
    world
        .entity_mut(busy)
        .insert(cdda_sim::creature::Goal {
            square: (3, 0, 0),
        });

    cdda_sim::turn::run_turn(&mut world);

    let place = world.get::<cdda_sim::Position>(busy).expect("it has a square");
    assert_eq!(
        (place.x, place.y),
        (0, 0),
        "a creature doing something spends its turn on that, not on walking"
    );
    assert!(moves_left(&world, busy) <= 0, "and it has no moves left to spend");
}
