//! What happens when something dies.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    Corpse, Monster, MonsterKindId, Position, corpses_at, occupied, reap_dead_monsters,
    spawn_monster, strike,
};

#[test]
fn a_dead_creature_leaves_a_corpse_on_its_square() {
    let mut world = World::new();
    let zombie = spawn_monster(&mut world, (3, 4), MonsterKindId(7), 5);
    assert!(strike(&mut world, zombie, 9, 0), "that killed it");
    assert_eq!(reap_dead_monsters(&mut world), 1, "one body cleared");
    assert_eq!(
        corpses_at(&mut world, (3, 4)),
        1,
        "and a corpse where it fell"
    );
    assert!(world.get_entity(zombie).is_err(), "the creature is gone");
}

#[test]
fn the_corpse_does_not_block_the_square() {
    let mut world = World::new();
    let zombie = spawn_monster(&mut world, (3, 4), MonsterKindId(7), 1);
    strike(&mut world, zombie, 5, 0);
    reap_dead_monsters(&mut world);
    assert!(
        !occupied(&mut world, (3, 4)),
        "a corpse is not a creature in the way"
    );
}

#[test]
fn a_living_creature_is_left_alone() {
    let mut world = World::new();
    let zombie = spawn_monster(&mut world, (0, 0), MonsterKindId(7), 5);
    assert_eq!(reap_dead_monsters(&mut world), 0, "nothing to clear");
    assert!(world.get_entity(zombie).is_ok());
    assert_eq!(corpses_at(&mut world, (0, 0)), 0);
}

#[test]
fn reaping_twice_does_not_duplicate_the_corpse() {
    let mut world = World::new();
    let zombie = spawn_monster(&mut world, (1, 1), MonsterKindId(7), 1);
    strike(&mut world, zombie, 5, 0);
    reap_dead_monsters(&mut world);
    assert_eq!(
        reap_dead_monsters(&mut world),
        0,
        "the second pass finds nothing"
    );
    assert_eq!(
        corpses_at(&mut world, (1, 1)),
        1,
        "and does not add a second corpse"
    );
}

#[test]
fn the_corpse_remembers_what_it_was() {
    let mut world = World::new();
    let zombie = spawn_monster(&mut world, (2, 2), MonsterKindId(42), 1);
    strike(&mut world, zombie, 5, 0);
    reap_dead_monsters(&mut world);
    let mut query = world.query::<(&Position, &Corpse)>();
    let kinds: Vec<MonsterKindId> = query.iter(&world).map(|(_, corpse)| corpse.kind).collect();
    assert_eq!(
        kinds,
        vec![MonsterKindId(42)],
        "the kind survives the death"
    );
}

#[test]
fn the_dead_stop_taking_turns() {
    let mut world = World::new();
    let zombie = spawn_monster(&mut world, (0, 0), MonsterKindId(7), 1);
    strike(&mut world, zombie, 5, 0);
    // The turn systems skip anything with no hit points, so this is about the corpse not becoming an actor.
    reap_dead_monsters(&mut world);
    let mut monsters = world.query::<&Monster>();
    assert_eq!(
        monsters.iter(&world).count(),
        0,
        "no creature is left to act"
    );
}
