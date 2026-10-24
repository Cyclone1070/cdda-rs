//! Taking charges out of a container that is lying on the square.
//!
//! Reference: `item::use_charges` (`src/item.cpp:13140-13165`) walks the square's items with `visit_items`, which visits an item **and then its contents**. So a canteen
//! on the floor is a source of its water, and the liquid gate that stops a puddle being used does not stop this one: that gate is asked about the **top-level** item
//! (`item_stack::use_charges`, `src/item_stack.cpp:150-155`), and a container is not a liquid.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    Container, GroundItem, ItemFacts, ItemTypeId, consume_items_held, contents, drop_item, insert_into,
};

const WATER: ItemTypeId = ItemTypeId(1);
const CANTEEN: ItemTypeId = ItemTypeId(2);
const ROCK: ItemTypeId = ItemTypeId(3);

fn volume_of(_item: Entity) -> ItemFacts {
    ItemFacts::solid(100)
}

fn charges_of(world: &mut World, entity: Entity) -> i32 {
    world.get::<GroundItem>(entity).expect("an item").charges
}

fn canteen(world: &mut World, water: i32) -> (Entity, Entity) {
    let canteen = drop_item(world, (3, 3), CANTEEN, 1);
    world.entity_mut(canteen).insert(Container::new(2_000));
    let water_item = drop_item(world, (3, 3), WATER, water);
    insert_into(world, canteen, water_item, volume_of).expect("the water fits");
    (canteen, water_item)
}

#[test]
fn the_water_inside_a_canteen_can_be_taken() {
    let mut world = World::new();
    let (canteen_entity, water_item) = canteen(&mut world, 6);
    let taken = consume_items_held(
        &mut world,
        (3, 3),
        WATER,
        4,
        |kind| kind == WATER,
        false,
        |_| true,
    );
    assert_eq!(taken.taken, 4, "the contents are a source");
    assert_eq!(charges_of(&mut world, water_item), 2);
    assert_eq!(
        contents(&mut world, canteen_entity),
        vec![water_item],
        "and the canteen still holds what is left"
    );
}

#[test]
fn an_emptied_pile_is_removed_from_its_container_and_the_container_is_left() {
    let mut world = World::new();
    let (canteen_entity, water_item) = canteen(&mut world, 6);
    let taken = consume_items_held(
        &mut world,
        (3, 3),
        WATER,
        6,
        |kind| kind == WATER,
        false,
        |_| true,
    );
    assert_eq!(taken.taken, 6);
    assert!(contents(&mut world, canteen_entity).is_empty(), "the water is gone");
    assert!(
        world.get::<GroundItem>(water_item).is_none(),
        "and the emptied pile went with it"
    );
    assert!(
        world.get::<GroundItem>(canteen_entity).is_some(),
        "the canteen is still there"
    );
}

#[test]
fn a_pile_later_on_the_floor_is_reached_after_the_canteen_that_lies_before_it() {
    // The visit is **pre-order**: the canteen is reached first, so its water is spent before the pile that lies on the floor beside it - even though that pile is a
    // closer match for a caller who only thinks about the square's own items. A port that walked the floor first and the contents afterwards would empty the wrong
    // one.
    let mut world = World::new();
    let (canteen_entity, water_item) = canteen(&mut world, 6);
    let floor_water = drop_item(&mut world, (3, 3), WATER, 10);
    // The square holds liquid, so the loose pile is usable too - and only then does the order decide which of the two is spent.
    let taken = consume_items_held(
        &mut world,
        (3, 3),
        WATER,
        8,
        |kind| kind == WATER,
        true,
        |_| true,
    );
    assert_eq!(taken.taken, 8);
    assert!(
        contents(&mut world, canteen_entity).is_empty(),
        "the canteen's six went first"
    );
    assert!(world.get::<GroundItem>(water_item).is_none());
    assert_eq!(
        charges_of(&mut world, floor_water),
        8,
        "and only the two that were still needed came off the floor"
    );
}

#[test]
fn a_rock_beside_the_canteen_is_still_taken_first_if_it_lies_first() {
    let mut world = World::new();
    let rock = drop_item(&mut world, (3, 3), ROCK, 2);
    let (canteen_entity, water_item) = canteen(&mut world, 6);
    let taken = consume_items_held(
        &mut world,
        (3, 3),
        ROCK,
        2,
        |kind| kind == WATER,
        false,
        |_| true,
    );
    assert_eq!(taken.taken, 2, "the rock on the floor");
    assert!(world.get::<GroundItem>(rock).is_none());
    assert_eq!(charges_of(&mut world, water_item), 6, "the water is untouched");
    assert_eq!(contents(&mut world, canteen_entity), vec![water_item]);
}
