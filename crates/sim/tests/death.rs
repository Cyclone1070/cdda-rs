//! What happens when something dies.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    CorpseOf, ItemTypeId, Monster, MonsterKindId, corpse_kind_at, corpses_at, items_at, occupied,
    reap_dead_monsters, spawn_monster, strike,
};

/// The item type the content calls `corpse`; the item layer numbers types rather than naming them.
const CORPSE: ItemTypeId = ItemTypeId(3_000);

#[test]
fn a_dead_creature_leaves_a_corpse_on_its_square() {
    let mut world = World::new();
    let zombie = spawn_monster(&mut world, (3, 4), MonsterKindId(7), 5);
    assert!(strike(&mut world, zombie, 9, 0), "that killed it");
    assert_eq!(
        reap_dead_monsters(&mut world, CORPSE),
        1,
        "one body cleared"
    );
    assert_eq!(
        corpses_at(&mut world, (3, 4)),
        1,
        "and a corpse where it fell"
    );
    assert!(world.get_entity(zombie).is_err(), "the creature is gone");
    assert_eq!(
        items_at(&mut world, (3, 4)).len(),
        1,
        "the corpse is an item, as the reference makes it"
    );
}

#[test]
fn the_corpse_does_not_block_the_square() {
    let mut world = World::new();
    let zombie = spawn_monster(&mut world, (3, 4), MonsterKindId(7), 1);
    strike(&mut world, zombie, 5, 0);
    reap_dead_monsters(&mut world, CORPSE);
    assert!(
        !occupied(&mut world, (3, 4)),
        "a corpse is not a creature in the way"
    );
}

#[test]
fn a_living_creature_is_left_alone() {
    let mut world = World::new();
    let zombie = spawn_monster(&mut world, (0, 0), MonsterKindId(7), 5);
    assert_eq!(
        reap_dead_monsters(&mut world, CORPSE),
        0,
        "nothing to clear"
    );
    assert!(world.get_entity(zombie).is_ok());
    assert_eq!(corpses_at(&mut world, (0, 0)), 0);
}

#[test]
fn reaping_twice_does_not_duplicate_the_corpse() {
    let mut world = World::new();
    let zombie = spawn_monster(&mut world, (1, 1), MonsterKindId(7), 1);
    strike(&mut world, zombie, 5, 0);
    reap_dead_monsters(&mut world, CORPSE);
    assert_eq!(
        reap_dead_monsters(&mut world, CORPSE),
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
    reap_dead_monsters(&mut world, CORPSE);
    assert_eq!(
        corpse_kind_at(&mut world, (2, 2)),
        Some(MonsterKindId(42)),
        "the kind survives the death"
    );
    let mut query = world.query::<&CorpseOf>();
    assert_eq!(
        query.iter(&world).count(),
        1,
        "and there is exactly one corpse item"
    );
}

#[test]
fn the_dead_stop_taking_turns() {
    let mut world = World::new();
    let zombie = spawn_monster(&mut world, (0, 0), MonsterKindId(7), 1);
    strike(&mut world, zombie, 5, 0);
    // The turn systems skip anything with no hit points, so this is about the corpse not becoming an actor.
    reap_dead_monsters(&mut world, CORPSE);
    let mut monsters = world.query::<&Monster>();
    assert_eq!(
        monsters.iter(&world).count(),
        0,
        "no creature is left to act"
    );
}
