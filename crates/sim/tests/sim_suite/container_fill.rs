//! What happens to a group's items that do not fit the container the group names.
//!
//! Reference: `put_into_container` (`src/item_group.cpp:115-171`). The rule is sharper than the names suggest: every drawn item is taken out of the group's list, then only
//! the ones that ended up in the container's `excess` pile are put back. So
//!
//! | `on_overflow` | what the player finds |
//! |---|---|
//! | `spill` | the item, loose on the ground |
//! | `discard` | nothing - it is gone on purpose |
//! | `none` | **nothing either**, and a diagnostic, because the data was wrong |
//!
//! A last item that is a **corpse** is forced in whatever its volume says (`ctr.is_corpse()`), which is why the reference has a branch for it rather than treating it as a
//! failed fit.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    Container, GroundItem, ItemFacts, ItemTypeId, Overflow, Position, drop_item, fill_container,
};

const BOX: ItemTypeId = ItemTypeId(9);
const ROCK: ItemTypeId = ItemTypeId(1);

/// Every item is 100 ml; a corpse is 100 ml too, so the volume is never the reason for the branch.
fn facts(_item: Entity) -> ItemFacts {
    ItemFacts::solid(100)
}

fn make_box(world: &mut World, capacity_ml: i64) -> Entity {
    let box_entity = drop_item(world, (5, 5), BOX, 1);
    world.entity_mut(box_entity).insert(Container::new(capacity_ml));
    box_entity
}

fn loose(world: &mut World) -> usize {
    let mut query = world.query::<(&Position, &GroundItem, Option<&cdda_sim::Inside>)>();
    query
        .iter(world)
        .filter(|(at, item, inside)| (at.x, at.y) == (5, 5) && inside.is_none() && item.kind == ROCK)
        .count()
}

#[test]
fn everything_fits_and_nothing_is_left_over() {
    let mut world = World::new();
    let box_entity = make_box(&mut world, 500);
    let rocks = vec![ROCK, ROCK];
    let filled = fill_container(&mut world, box_entity, &rocks, Overflow::Spill, facts);
    assert_eq!(filled.inside, 2, "both rocks went in");
    assert_eq!(filled.left_over, 0);
    assert_eq!(loose(&mut world), 0);
}

#[test]
fn spill_leaves_the_leftovers_on_the_ground() {
    let mut world = World::new();
    let box_entity = make_box(&mut world, 150);
    let rocks = vec![ROCK, ROCK, ROCK];
    let filled = fill_container(&mut world, box_entity, &rocks, Overflow::Spill, facts);
    assert_eq!(filled.inside, 1, "one rock fits");
    assert_eq!(filled.left_over, 2);
    assert_eq!(loose(&mut world), 2, "and the two that did not are on the square");
}

#[test]
fn discard_loses_the_leftovers_on_purpose() {
    let mut world = World::new();
    let box_entity = make_box(&mut world, 150);
    let rocks = vec![ROCK, ROCK, ROCK];
    let filled = fill_container(&mut world, box_entity, &rocks, Overflow::Discard, facts);
    assert_eq!(filled.inside, 1);
    assert_eq!(filled.left_over, 2, "the player is told what it cost");
    assert_eq!(loose(&mut world), 0, "and nothing is left on the ground");
}

#[test]
fn none_loses_the_leftovers_too_because_the_data_was_wrong() {
    // The reference reports a debug message and then drops them, because `none` means "the data should have fitted" rather than "keep it somewhere".
    let mut world = World::new();
    let box_entity = make_box(&mut world, 150);
    let rocks = vec![ROCK, ROCK, ROCK];
    let filled = fill_container(&mut world, box_entity, &rocks, Overflow::None, facts);
    assert_eq!(filled.inside, 1);
    assert_eq!(filled.left_over, 2);
    assert_eq!(loose(&mut world), 0, "the same outcome as a discard, and a diagnostic");
}
