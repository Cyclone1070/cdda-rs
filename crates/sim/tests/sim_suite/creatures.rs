//! A creature is an entity standing on a square, like everything else.
//!
//! Reference: `Creature` (`src/creature.h`) and its two subclasses are a class hierarchy with a
//! virtual `move`; the port does not take the hierarchy, only what a creature *has*: a place, a body,
//! a pool of action points, and the name it is known by. Everything that happens to a creature is a
//! system that reads those components, and nothing about it lives in a side table.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::creature::{Creature, Moves, creature_at, give_moves, spawn_creature};
use cdda_sim::{Position, singletons};

#[test]
fn a_creature_stands_on_a_square_and_the_index_knows_it() {
    let mut world = World::new();
    let you = spawn_creature(&mut world, "the survivor", (5, 5, 0));

    assert_eq!(
        creature_at(&world, (5, 5, 0)),
        vec![you],
        "the creature is found where it stands"
    );
    assert!(
        world
            .get::<Position>(you)
            .is_some_and(|position| (position.x, position.y, position.z) == (5, 5, 0)),
        "and it carries the square rather than being filed under it"
    );
    assert!(
        singletons::index(&world).is_some_and(|index| index.at_level(5, 5, 0).contains(&you)),
        "the index is built from the store, so a creature in the store is in the index"
    );
}

#[test]
fn action_points_are_the_creature_s_own() {
    let mut world = World::new();
    let you = spawn_creature(&mut world, "the survivor", (5, 5, 0));
    let other = spawn_creature(&mut world, "a dog", (6, 5, 0));

    give_moves(&mut world, you, 100);
    assert_eq!(moves_of(&world, you), 100, "the points were given to it");
    assert_eq!(moves_of(&world, other), 0, "and not to the dog beside it");
}

#[test]
fn a_creature_carries_its_own_name_and_place_in_the_world() {
    let mut world = World::new();
    let you = spawn_creature(&mut world, "the survivor", (5, 5, 0));
    let dog = spawn_creature(&mut world, "a dog", (5, 5, 0));

    assert_eq!(
        creature_at(&world, (5, 5, 0)).len(),
        2,
        "two creatures can stand on one square"
    );
    let names: Vec<String> = [you, dog]
        .iter()
        .filter_map(|entity| world.get::<Creature>(*entity).map(|one| one.name.clone()))
        .collect();
    assert_eq!(names, vec!["the survivor", "a dog"], "each keeps its own name");
}

/// The action points a creature has left.
fn moves_of(world: &World, entity: Entity) -> i32 {
    world.get::<Moves>(entity).map_or(0, |moves| moves.left)
}
