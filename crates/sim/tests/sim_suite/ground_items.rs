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

/// Faults come off one at a time, and the singular in the name is the rule.
///
/// Reference: `item::remove_single_fault_of_type` (`item.cpp:7874-7882`): it walks the item's faults and `return`s on the first whose type matches - **an item with two mechanical faults loses
/// one per call, not both**, which is what a caller clearing a blow's damage would get wrong.
#[test]
fn one_fault_of_a_type_comes_off_per_call() {
    use cdda_sim::{ItemFaults, remove_one_fault_of_type};

    let mut faults = ItemFaults::default();
    for fault in ["dented", "cracked", "wet"] {
        faults.0.insert(fault.to_owned());
    }
    // The fault catalogue says what each id is; this crate does not own it.
    let kinds = |fault: &str| Some(if fault == "wet" { "wet" } else { "mechanical_damage" }.to_owned());

    let first = remove_one_fault_of_type(&mut faults, kinds, "mechanical_damage");
    assert!(first.is_some(), "one came off");
    assert_eq!(faults.0.len(), 2, "**one**, not both mechanical faults");
    let second = remove_one_fault_of_type(&mut faults, kinds, "mechanical_damage");
    assert!(second.is_some() && second != first, "the next call takes the other");
    assert!(remove_one_fault_of_type(&mut faults, kinds, "mechanical_damage").is_none(), "then none is left");
    assert_eq!(faults.0.len(), 1, "and the wet fault was never touched");
    assert_eq!(remove_one_fault_of_type(&mut faults, kinds, "wet").as_deref(), Some("wet"), "which the wet type still finds");
}
