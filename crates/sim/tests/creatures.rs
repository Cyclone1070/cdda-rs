//! Creatures on the map: where they stand, and what damage does to them.
//!
//! Reference: `monster` (`src/monster.h`), and the death check after `Creature::deal_damage`. Only
//! the parts a field processor needs are here; behaviour, speed and armour come with the systems
//! that use them.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    ItemTypeId, Monster, MonsterKindId, damage_monster, drop_item, items_at, monsters_at,
    spawn_monster,
};

#[test]
fn a_creature_is_where_it_was_put() {
    let mut world = World::new();
    let entity = spawn_monster(&mut world, (5, 5), MonsterKindId(1), 20);
    assert_eq!(monsters_at(&mut world, (5, 5)), vec![entity]);
    assert!(
        monsters_at(&mut world, (5, 6)).is_empty(),
        "and nowhere else"
    );
}

#[test]
fn damage_comes_off_the_hit_points() {
    let mut world = World::new();
    let entity = spawn_monster(&mut world, (5, 5), MonsterKindId(1), 20);
    assert!(
        !damage_monster(&mut world, entity, 8),
        "eight off twenty is survivable"
    );
    assert_eq!(world.get::<Monster>(entity).map(|m| m.hp), Some(12));
}

#[test]
fn enough_damage_takes_the_creature_off_the_map() {
    let mut world = World::new();
    let entity = spawn_monster(&mut world, (5, 5), MonsterKindId(1), 20);
    assert!(damage_monster(&mut world, entity, 20), "exactly lethal");
    assert!(world.get::<Monster>(entity).is_none(), "and gone");
    assert!(monsters_at(&mut world, (5, 5)).is_empty());
}

#[test]
fn negative_damage_heals_nothing_and_does_not_kill() {
    let mut world = World::new();
    let entity = spawn_monster(&mut world, (5, 5), MonsterKindId(1), 5);
    assert!(
        !damage_monster(&mut world, entity, -10),
        "a negative amount is not a heal"
    );
    assert_eq!(world.get::<Monster>(entity).map(|m| m.hp), Some(5));
}

#[test]
fn two_creatures_on_one_square_keep_their_spawn_order() {
    let mut world = World::new();
    let first = spawn_monster(&mut world, (5, 5), MonsterKindId(1), 10);
    let second = spawn_monster(&mut world, (5, 5), MonsterKindId(2), 10);
    assert_eq!(monsters_at(&mut world, (5, 5)), vec![first, second]);
}

#[test]
fn creatures_and_items_share_a_square_without_seeing_each_other() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    let creature = spawn_monster(&mut world, (5, 5), MonsterKindId(1), 10);
    assert_eq!(monsters_at(&mut world, (5, 5)), vec![creature]);
    assert_eq!(
        items_at(&mut world, (5, 5)).len(),
        1,
        "and the item is still there"
    );
}
