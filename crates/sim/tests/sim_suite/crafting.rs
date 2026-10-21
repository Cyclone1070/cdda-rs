//! Making something from what is on the floor, all or nothing.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    CraftError, GroundItem, ItemTypeId, consume_items, craft_on_square, drop_item, items_at,
};

#[test]
fn a_satisfied_recipe_takes_its_parts_and_leaves_the_result() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 3);
    drop_item(&mut world, (5, 5), ItemTypeId(2), 1);
    let crafted = craft_on_square(
        &mut world,
        (5, 5),
        &[vec![(ItemTypeId(1), 2)]],
        1,
        ItemTypeId(9),
    )
    .expect("the parts are there");
    assert_eq!(crafted.count, 1);
    assert_eq!(
        world
            .get::<GroundItem>(crafted.result)
            .map(|item| item.kind),
        Some(ItemTypeId(9))
    );
    let left: i32 = items_at(&mut world, (5, 5))
        .into_iter()
        .filter(|entity| {
            world.get::<GroundItem>(*entity).map(|item| item.kind) == Some(ItemTypeId(1))
        })
        .filter_map(|entity| world.get::<GroundItem>(entity).map(|item| item.charges))
        .sum();
    assert_eq!(left, 1, "one of the three was left alone");
}

#[test]
fn a_missing_group_means_nothing_happens_at_all() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 5);
    let error = craft_on_square(
        &mut world,
        (5, 5),
        &[vec![(ItemTypeId(1), 2)], vec![(ItemTypeId(2), 1)]],
        1,
        ItemTypeId(9),
    )
    .expect_err("the second part is missing");
    assert_eq!(error, CraftError::Missing { group: 1 });
    assert_eq!(
        consume_items(&mut world, (5, 5), ItemTypeId(1), 5).taken,
        5,
        "the first group was put back: all five are still there"
    );
}

#[test]
fn a_batch_multiplies_what_is_taken_and_what_is_made() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 10);
    let crafted = craft_on_square(
        &mut world,
        (5, 5),
        &[vec![(ItemTypeId(1), 3)]],
        2,
        ItemTypeId(9),
    )
    .expect("ten covers six");
    assert_eq!(crafted.count, 2);
    assert_eq!(
        consume_items(&mut world, (5, 5), ItemTypeId(1), 100).taken,
        4,
        "six of the ten were spent"
    );
}

#[test]
fn the_first_alternative_that_can_be_met_is_the_one_used() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(2), 4);
    let crafted = craft_on_square(
        &mut world,
        (5, 5),
        &[vec![(ItemTypeId(1), 2), (ItemTypeId(2), 2)]],
        1,
        ItemTypeId(9),
    )
    .expect("the second alternative is there");
    assert_eq!(crafted.count, 1);
    assert_eq!(
        consume_items(&mut world, (5, 5), ItemTypeId(2), 4).taken,
        2,
        "the second alternative was spent, and only by two"
    );
}

#[test]
fn a_short_alternative_is_put_back_when_the_next_one_fills_the_group() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    drop_item(&mut world, (5, 5), ItemTypeId(2), 5);
    let crafted = craft_on_square(
        &mut world,
        (5, 5),
        &[vec![(ItemTypeId(1), 2), (ItemTypeId(2), 2)]],
        1,
        ItemTypeId(9),
    )
    .expect("two of the second kind are there");
    assert_eq!(crafted.count, 1);
    assert_eq!(
        consume_items(&mut world, (5, 5), ItemTypeId(1), 1).taken,
        1,
        "not a thing of the first kind was lost"
    );
}
