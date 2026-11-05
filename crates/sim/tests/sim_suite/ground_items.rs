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

/// A damaged weapon can pick up a fault, and the pick is weighted, filtered and often empty.
///
/// Reference: `item::set_random_fault_of_type` (`item.cpp:7848-7868`). Three behaviours, and the third is the one a port invents: **a list of faults the item cannot have sets no fault at
/// all** - the reference does not fall back to forcing one, it does nothing.
#[test]
fn a_fault_is_picked_by_weight_and_only_from_what_the_item_can_have() {
    use cdda_core_types::rng::Rolls;
    use cdda_sim::pick_a_fault;

    // Two mechanical faults, one three times as likely as the other: three quarters against one quarter.
    let candidates = vec![("dented".to_owned(), 3), ("cracked".to_owned(), 1)];
    let mut dice = Rolls::new(99);
    let mut dented = 0;
    for _ in 0..4_000 {
        match pick_a_fault(&candidates, &mut dice).as_deref() {
            Some("dented") => dented += 1,
            Some("cracked") => {}
            other => panic!("only the two candidates can come back: {other:?}"),
        }
    }
    assert!((2_800..3_200).contains(&dented), "three to one: {dented} of 4000 dented");

    // A single candidate always wins, because the roll has nowhere else to go.
    let only = vec![("dented".to_owned(), 5)];
    let mut dice = Rolls::new(1);
    for _ in 0..100 {
        assert_eq!(pick_a_fault(&only, &mut dice).as_deref(), Some("dented"));
    }

    // Nothing eligible, nothing set - the reference's silent no-op.
    let mut dice = Rolls::new(1);
    assert_eq!(pick_a_fault(&[], &mut dice), None, "an empty list sets no fault");
    assert_eq!(pick_a_fault(&[("dented".to_owned(), 0)], &mut dice), None, "and a zero weight is no weight");
}
