//! Items lying on a square: what is there, and in what order.
//!
//! Reference: `map_stack` (`src/map.cpp`), which keeps a tile's items in the order they were added.
//! The order is not cosmetic: a fire that burns a pile consumes it front to back, so the order is
//! part of what a replay has to reproduce.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::{GroundItem, ItemTypeId, drop_item, items_at, take_item};

fn kind(n: u32) -> ItemTypeId {
    ItemTypeId(n)
}

#[test]
fn a_square_hands_back_what_was_put_on_it_in_order() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), kind(1), 1);
    drop_item(&mut world, (5, 5), kind(2), 3);
    let third = drop_item(&mut world, (5, 5), kind(3), 1);
    let items = items_at(&mut world, (5, 5));
    let kinds: Vec<u32> = items
        .iter()
        .map(|entity| world.get::<GroundItem>(*entity).expect("an item").kind.0)
        .collect();
    assert_eq!(kinds, vec![1, 2, 3], "insertion order");
    assert_eq!(items.last(), Some(&third), "the most recent is last");
}

#[test]
fn an_empty_square_holds_nothing() {
    let mut world = World::new();
    assert!(items_at(&mut world, (1, 1)).is_empty());
}

#[test]
fn a_square_only_returns_its_own_items() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), kind(1), 1);
    drop_item(&mut world, (6, 5), kind(2), 1);
    assert_eq!(
        items_at(&mut world, (5, 5)).len(),
        1,
        "the neighbour's item stays put"
    );
    assert_eq!(items_at(&mut world, (6, 5)).len(), 1);
}

#[test]
fn charges_survive_the_round_trip() {
    let mut world = World::new();
    let entity = drop_item(&mut world, (5, 5), kind(7), 42);
    let item = world.get::<GroundItem>(entity).expect("an item");
    assert_eq!(item.charges, 42);
}

#[test]
fn a_taken_item_is_gone_and_the_rest_stay_in_order() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), kind(1), 1);
    let middle = drop_item(&mut world, (5, 5), kind(2), 1);
    drop_item(&mut world, (5, 5), kind(3), 1);
    take_item(&mut world, middle);
    let remaining: Vec<u32> = items_at(&mut world, (5, 5))
        .iter()
        .map(|entity| world.get::<GroundItem>(*entity).expect("an item").kind.0)
        .collect();
    assert_eq!(remaining, vec![1, 3], "what is left keeps its order");
}
