//! Taking **items**, which is a different function from taking charges and does not look alike.
//!
//! Reference: `map::use_amount_square` → `use_amount_stack` → `item::use_amount` (`src/map.cpp:5889-5902, 5903-5920`, `src/item.cpp:12868-12921`). Asking for *amounts*
//! removes whole items - `quantity--` per item, whatever the item holds - while `map::use_charges` removes charges and leaves the item behind. The visit order differs
//! too: `use_amount` walks a container's contents **first**, from the bottom up, and the container itself last, which is the reverse of the charges walk.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    Container, GroundItem, ItemFacts, ItemTypeId, Position, consume_items,
    consume_items_by_amount, contents, drop_item, insert_into,
};

const WATER: ItemTypeId = ItemTypeId(1);
const CANTEEN: ItemTypeId = ItemTypeId(2);

fn volume_of(_item: Entity) -> ItemFacts {
    ItemFacts::solid(100)
}

fn piles(world: &mut World) -> Vec<(ItemTypeId, i32)> {
    let mut query = world.query::<(&Position, &GroundItem)>();
    query
        .iter(world)
        .filter(|(at, _)| (at.x, at.y) == (3, 3))
        .map(|(_, item)| (item.kind, item.charges))
        .collect()
}

#[test]
fn an_amount_takes_the_whole_item_and_a_charge_takes_one_charge() {
    // The same pile, asked the same question, under the two functions.
    let mut world = World::new();
    drop_item(&mut world, (3, 3), WATER, 5);
    let taken = consume_items_by_amount(&mut world, (3, 3), WATER, 1, |_| true);
    assert_eq!(taken.taken, 1, "one item, not one charge");
    assert!(piles(&mut world).is_empty(), "and all five charges went with it");

    let mut world = World::new();
    drop_item(&mut world, (3, 3), WATER, 5);
    let taken = consume_items(&mut world, (3, 3), WATER, 1);
    assert_eq!(taken.taken, 1, "one charge");
    assert_eq!(piles(&mut world), vec![(WATER, 4)], "the pile stays");
}

#[test]
fn a_filter_an_amount_rejects_is_not_consumed() {
    let mut world = World::new();
    drop_item(&mut world, (3, 3), WATER, 5);
    let taken = consume_items_by_amount(&mut world, (3, 3), WATER, 1, |_| false);
    assert_eq!(taken.taken, 0);
    assert!(!taken.satisfied);
    assert_eq!(piles(&mut world), vec![(WATER, 5)]);
}

#[test]
fn an_amount_reaches_inside_a_container_before_it_reaches_the_container() {
    // The canteen holds water and the square is asked for one **water**: the contents are visited first, so the water goes and the canteen stays.
    let mut world = World::new();
    let canteen = drop_item(&mut world, (3, 3), CANTEEN, 1);
    world.entity_mut(canteen).insert(Container::new(2_000));
    let water_item = drop_item(&mut world, (3, 3), WATER, 6);
    insert_into(&mut world, canteen, water_item, volume_of).expect("the water fits");
    let taken = consume_items_by_amount(&mut world, (3, 3), WATER, 1, |_| true);
    assert_eq!(taken.taken, 1);
    assert!(contents(&mut world, canteen).is_empty(), "the water is gone");
    assert_eq!(
        piles(&mut world),
        vec![(CANTEEN, 1)],
        "and the canteen is still on the square"
    );
}

#[test]
fn an_amount_of_the_container_itself_takes_what_is_inside_with_it() {
    let mut world = World::new();
    let canteen = drop_item(&mut world, (3, 3), CANTEEN, 1);
    world.entity_mut(canteen).insert(Container::new(2_000));
    let water_item = drop_item(&mut world, (3, 3), WATER, 6);
    insert_into(&mut world, canteen, water_item, volume_of).expect("the water fits");
    let taken = consume_items_by_amount(&mut world, (3, 3), CANTEEN, 1, |_| true);
    assert_eq!(taken.taken, 1);
    assert!(
        piles(&mut world).is_empty(),
        "the canteen went, and its water with it"
    );
    assert!(world.get::<GroundItem>(water_item).is_none());
}

/// Inside a container the walk goes **bottom up**: the last thing put in is the first thing taken.
///
/// Reference: `item::use_amount` (`src/item.cpp:12877-12888`) reverses the list it gets - "as it's created from the top down, but we have to remove items from the bottom up
/// in order for the references to remain valid" - so the order is observable whenever two contents of the same kind could both satisfy the request.
#[test]
fn the_last_thing_put_into_a_container_is_the_first_thing_taken() {
    let mut world = World::new();
    let canteen = drop_item(&mut world, (3, 3), CANTEEN, 1);
    world.entity_mut(canteen).insert(Container::new(2_000));
    let first = drop_item(&mut world, (3, 3), WATER, 6);
    insert_into(&mut world, canteen, first, volume_of).expect("it fits");
    let second = drop_item(&mut world, (3, 3), WATER, 4);
    insert_into(&mut world, canteen, second, volume_of).expect("it fits");
    let taken = consume_items_by_amount(&mut world, (3, 3), WATER, 1, |_| true);
    assert_eq!(taken.taken, 1);
    assert_eq!(
        contents(&mut world, canteen),
        vec![first],
        "the pile put in last went first"
    );
    assert!(world.get::<GroundItem>(second).is_none());
}
