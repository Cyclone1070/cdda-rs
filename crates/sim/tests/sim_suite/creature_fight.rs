//! A creature that reaches its goal hits what is standing there.
//!
//! Reference: `monster::move_to` refuses a square another creature holds, and `monster::attack` is what a
//! monster does instead - the swing is the move. These tests drive the **turn**, not `strike` directly, so
//! what is checked is that the behaviour is wired into the declared stage rather than existing as a function
//! nobody calls.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_core_types::rng::Rolls;
use cdda_sim::creature::{
    MeleeAttack, creature_at, health_of, set_goal, set_health, set_speed, spawn_creature,
};
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{Prototypes, singletons};

const FLOOR: u32 = 1;

/// A one-row corridor of floor, with the world's dice seeded so the same seed gives the same fight.
fn corridor(length: i32, seed: u32) -> World {
    let mut world = World::new();
    let mut level = Level::new();
    for x in 0..length {
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
    singletons::put_rolls(&mut world, Rolls::new(seed));
    world
}

/// Runs one creature stage, which is what asks creatures to act.
fn a_turn(world: &mut World) {
    cdda_sim::turn::run_turn(world);
}

fn zombie(world: &mut World, square: (i32, i32, i32), hp: i32) -> Entity {
    let entity = spawn_creature(world, "a zombie", square);
    set_health(world, entity, hp);
    set_speed(world, entity, 100, 0);
    // Two dice of three faces, at skill four - the corpus zombie's own numbers.
    world.entity_mut(entity).insert(MeleeAttack(cdda_sim::melee::Attack {
        accuracy_tenths: 40,
        fixed: cdda_sim::melee::Damage::default(),
        dice: 2,
        sides: 3,
    }));
    entity
}

#[test]
fn a_creature_whose_goal_is_in_reach_hits_instead_of_walking() {
    let mut world = corridor(3, 1);
    let zombie_entity = zombie(&mut world, (0, 0, 0), 20);
    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    set_health(&mut world, survivor, 30);
    set_goal(&mut world, zombie_entity, (1, 0, 0));

    a_turn(&mut world);

    let taken = 30 - health_of(&world, survivor).expect("the survivor is alive");
    assert!(
        (2..=6).contains(&taken),
        "two dice of three faces take two to six, took {taken}"
    );
    assert_eq!(
        creature_at(&world, (0, 0, 0)),
        vec![zombie_entity],
        "and the zombie stayed where it was: the swing was its move"
    );
}

#[test]
fn a_creature_whose_goal_is_far_away_walks_toward_it() {
    let mut world = corridor(4, 1);
    let zombie_entity = zombie(&mut world, (0, 0, 0), 20);
    let survivor = spawn_creature(&mut world, "the survivor", (3, 0, 0));
    set_health(&mut world, survivor, 30);
    set_goal(&mut world, zombie_entity, (3, 0, 0));

    a_turn(&mut world);

    assert_eq!(
        creature_at(&world, (1, 0, 0)),
        vec![zombie_entity],
        "it stepped one square closer"
    );
    assert_eq!(
        health_of(&world, survivor),
        Some(30),
        "and nobody was hurt at that distance"
    );
}

#[test]
fn a_creature_with_no_attack_never_hurts_anyone() {
    let mut world = corridor(3, 1);
    let walker = spawn_creature(&mut world, "a walker", (0, 0, 0));
    set_health(&mut world, walker, 20);
    set_speed(&mut world, walker, 100, 0);
    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    set_health(&mut world, survivor, 30);
    set_goal(&mut world, walker, (1, 0, 0));

    a_turn(&mut world);

    assert_eq!(
        health_of(&world, survivor),
        Some(30),
        "a creature with no melee attack cannot wound anyone, however close it stands"
    );
}

#[test]
fn a_creature_with_no_goal_does_nothing() {
    let mut world = corridor(3, 1);
    let zombie_entity = zombie(&mut world, (0, 0, 0), 20);
    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    set_health(&mut world, survivor, 30);

    a_turn(&mut world);

    assert_eq!(creature_at(&world, (0, 0, 0)), vec![zombie_entity], "it stayed");
    assert_eq!(health_of(&world, survivor), Some(30), "and nobody was hit");
}
