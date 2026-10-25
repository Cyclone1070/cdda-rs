//! The two questions a pocket asks that the port was not asking: how long, and how small.
//!
//! Reference: `item_pocket::is_compatible` (`src/item_pocket.cpp:1408-1432`), which every `can_contain` runs **before** the weight and the room: an item longer than
//! `max_contains_length` is `"item is too long"`, one shorter than `min_contains_length` is `"item is too short"`, and one below `min_contains_volume` is `"item is too
//! small"`. The order is the interesting part - a long item that is also too heavy must be refused for being long - and these tests pin it, because a port that checked
//! weight first would report the wrong refusal and, once messages exist, the wrong message.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_sim::{Container, InsertError, ItemFacts, ItemTypeId, Pocket, drop_item, insert_into};

/// A 1,000 ml solid that is 100 mm long.
fn chunky(_item: Entity) -> ItemFacts {
    ItemFacts {
        length_mm: 100,
        ..ItemFacts::solid_weighing(1_000, 900)
    }
}

/// A pocket of a given volume that takes anything at all.
fn pocket(capacity_ml: i64) -> Pocket {
    Pocket::of(capacity_ml)
}

fn box_with(pockets: Vec<Pocket>) -> (World, Entity, Entity) {
    let mut world = World::new();
    let box_entity = drop_item(&mut world, (5, 5), ItemTypeId(9), 1);
    world.entity_mut(box_entity).insert(Container::with_pockets(pockets));
    let item = drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    (world, box_entity, item)
}

#[test]
fn an_item_longer_than_the_pocket_allows_is_refused_for_being_too_long() {
    let (mut world, box_entity, item) = box_with(vec![Pocket {
        max_item_length_mm: 50,
        ..pocket(5_000)
    }]);
    assert_eq!(
        insert_into(&mut world, box_entity, item, chunky),
        Err(InsertError::TooLong {
            limit_mm: 50,
            wanted_mm: 100
        })
    );
}

#[test]
fn the_length_is_asked_before_the_weight() {
    // The same item is too long **and** too heavy; the reference reports the length, because compatibility runs first.
    let (mut world, box_entity, item) = box_with(vec![Pocket {
        weight_capacity_grams: 100,
        max_item_length_mm: 50,
        ..pocket(5_000)
    }]);
    assert_eq!(
        insert_into(&mut world, box_entity, item, chunky),
        Err(InsertError::TooLong {
            limit_mm: 50,
            wanted_mm: 100
        }),
        "the length is asked before the weight"
    );
}

#[test]
fn an_item_shorter_than_the_pocket_allows_is_refused_for_being_too_short() {
    let (mut world, box_entity, item) = box_with(vec![Pocket {
        min_item_length_mm: 200,
        ..pocket(5_000)
    }]);
    assert_eq!(
        insert_into(&mut world, box_entity, item, chunky),
        Err(InsertError::TooShort {
            limit_mm: 200,
            wanted_mm: 100
        })
    );
}

#[test]
fn an_item_below_the_pocket_s_minimum_volume_is_refused_for_being_too_small() {
    let (mut world, box_entity, item) = box_with(vec![Pocket {
        min_item_volume_ml: 1_500,
        ..pocket(5_000)
    }]);
    assert_eq!(
        insert_into(&mut world, box_entity, item, chunky),
        Err(InsertError::TooSmall {
            limit_ml: 1_500,
            wanted_ml: 1_000
        })
    );
}

#[test]
fn a_pocket_without_limits_still_takes_the_item() {
    let (mut world, box_entity, item) = box_with(vec![pocket(5_000)]);
    assert_eq!(insert_into(&mut world, box_entity, item, chunky), Ok(()));
}

#[test]
fn a_liquid_is_not_measured_for_length() {
    // "liquids and gases avoid the size limit altogether" (`src/item_pocket.cpp:1409-1410`): a 1 litre of water in a watertight pocket with a 50 mm limit goes in.
    let mut world = World::new();
    let bottle = drop_item(&mut world, (5, 5), ItemTypeId(9), 1);
    world.entity_mut(bottle).insert(Container::with_pockets(vec![Pocket {
        watertight: true,
        max_item_length_mm: 50,
        ..pocket(5_000)
    }]));
    let water = drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    assert_eq!(
        insert_into(&mut world, bottle, water, |_| ItemFacts::liquid(1_000)),
        Ok(())
    );
}
