//! A creature is an entity standing on a square, like everything else.
//!
//! Reference: `Creature` (`src/creature.h`) and its two subclasses are a class hierarchy with a
//! virtual `move`; the port does not take the hierarchy, only what a creature *has*: a place, a body,
//! a pool of action points, and the name it is known by. Everything that happens to a creature is a
//! system that reads those components, and nothing about it lives in a side table.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::creature::{
    Creature, Moves, bank_a_turn, creature_at, give_moves, scaled_speed, set_speed, spawn_creature,
};
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

#[test]
fn a_creature_banks_its_speed_in_action_points_each_turn() {
    let mut world = World::new();
    let quick = spawn_creature(&mut world, "a runner", (5, 5, 0));
    let slow = spawn_creature(&mut world, "a shambler", (6, 5, 0));
    set_speed(&mut world, quick, 100, 0);
    set_speed(&mut world, slow, 50, 0);

    bank_a_turn(&mut world);

    assert_eq!(moves_of(&world, quick), 100, "the quick one banks its whole speed");
    assert_eq!(moves_of(&world, slow), 50, "the slow one banks half of it");

    bank_a_turn(&mut world);
    assert_eq!(moves_of(&world, quick), 200, "and points accumulate while unspent");
}

/// Speed as the reference scales it: `mon.speed *= MONSTER_SPEED / 100.0` on an `int`
/// (`monstergenerator.cpp:346`), so the product truncates toward zero.
#[test]
fn the_monster_speed_option_scales_speed_and_truncates() {
    assert_eq!(scaled_speed(100, 100), 100, "a hundred percent is unchanged");
    assert_eq!(scaled_speed(100, 150), 150, "a half again");
    assert_eq!(scaled_speed(100, 33), 33, "a third, truncated");
    assert_eq!(scaled_speed(130, 75), 97, "97.5 truncates to 97");
    assert_eq!(scaled_speed(70, 100), 70, "and a speed that needs no scaling keeps its value");
}

/// The action points a creature has left.
fn moves_of(world: &World, entity: Entity) -> i32 {
    world.get::<Moves>(entity).map_or(0, |moves| moves.left)
}
