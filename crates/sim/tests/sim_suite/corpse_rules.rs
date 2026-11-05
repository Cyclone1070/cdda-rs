//! What makes a corpse a corpse, and when it is too late to use it.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    CorpseOf, GroundItem, ItemTypeId, MonsterKindId, TURNS_PER_DAY, corpse_age, corpse_rotten_away,
    drop_item, is_corpse,
};

const CORPSE: ItemTypeId = ItemTypeId(3_000);
const ROCK: ItemTypeId = ItemTypeId(1_000);

fn flagged(kind: ItemTypeId) -> bool {
    kind == CORPSE
}

fn corpse(world: &mut World, age: i64) -> Entity {
    let item = drop_item(world, (0, 0), CORPSE, 1);
    world.entity_mut(item).insert(CorpseOf {
        kind: MonsterKindId(9),
        died_on: 0,
        revives: false,
        monster_volume_ml: 62_499,
    });
    world.get_mut::<GroundItem>(item).expect("an item").age = age;
    item
}

#[test]
fn a_corpse_is_an_item_that_names_a_creature_and_carries_the_flag() {
    let mut world = World::new();
    let body = corpse(&mut world, 0);
    assert!(is_corpse(&mut world, body, flagged));
}

#[test]
fn an_item_that_names_no_creature_is_not_a_corpse() {
    let mut world = World::new();
    let rock = drop_item(&mut world, (0, 0), ROCK, 1);
    assert!(!is_corpse(&mut world, rock, flagged));
}

#[test]
fn naming_a_creature_is_not_enough_without_the_flag() {
    let mut world = World::new();
    let body = corpse(&mut world, 0);
    assert!(
        !is_corpse(&mut world, body, |_| false),
        "the item type has to say it too"
    );
}

#[test]
fn a_corpse_knows_how_long_it_has_been_lying_there() {
    let mut world = World::new();
    let body = corpse(&mut world, 3 * TURNS_PER_DAY);
    assert_eq!(corpse_age(&mut world, body), Some(3 * TURNS_PER_DAY));
}

#[test]
fn ten_days_is_the_line_and_it_is_strict() {
    let mut world = World::new();
    let exactly = corpse(&mut world, 10 * TURNS_PER_DAY);
    assert!(
        !corpse_rotten_away(&mut world, exactly, false),
        "ten days is not more than ten"
    );
    let longer = corpse(&mut world, 10 * TURNS_PER_DAY + 1);
    assert!(corpse_rotten_away(&mut world, longer, false));
}

#[test]
fn a_corpse_that_can_revive_does_not_rot_away() {
    let mut world = World::new();
    let body = corpse(&mut world, 100 * TURNS_PER_DAY);
    assert!(
        !corpse_rotten_away(&mut world, body, true),
        "it is waiting for something else"
    );
}

#[test]
fn an_item_that_is_not_a_corpse_never_rots_away_here() {
    let mut world = World::new();
    let rock = drop_item(&mut world, (0, 0), ROCK, 1);
    assert_eq!(corpse_age(&mut world, rock), None);
    assert!(!corpse_rotten_away(&mut world, rock, false));
}
