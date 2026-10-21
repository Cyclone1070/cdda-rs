//! Taking things off the floor, which is what crafting and eating both start with.
//!
//! Transcribed from `map::use_amount_square` (`src/map.cpp:5877-5897`).

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::{Consumed, GroundItem, ItemTypeId, consume_items, drop_item, items_at};

#[test]
fn a_partial_take_leaves_the_rest_of_the_stack() {
    let mut world = World::new();
    let pile = drop_item(&mut world, (5, 5), ItemTypeId(1), 10);
    let result = consume_items(&mut world, (5, 5), ItemTypeId(1), 4);
    assert_eq!(
        result,
        Consumed {
            taken: 4,
            satisfied: true
        }
    );
    assert_eq!(
        world.get::<GroundItem>(pile).map(|item| item.charges),
        Some(6)
    );
}

#[test]
fn taking_exactly_what_is_there_empties_the_square() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 10);
    let result = consume_items(&mut world, (5, 5), ItemTypeId(1), 10);
    assert_eq!(
        result,
        Consumed {
            taken: 10,
            satisfied: true
        }
    );
    assert!(items_at(&mut world, (5, 5)).is_empty(), "the pile is gone");
}

#[test]
fn asking_for_more_than_there_is_takes_everything_and_says_it_came_up_short() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 3);
    let result = consume_items(&mut world, (5, 5), ItemTypeId(1), 12);
    assert_eq!(
        result,
        Consumed {
            taken: 3,
            satisfied: false
        },
        "the caller must know it only got three, so it can put them back"
    );
    assert!(items_at(&mut world, (5, 5)).is_empty());
}

#[test]
fn other_things_on_the_square_are_left_alone() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 2);
    let battery = drop_item(&mut world, (5, 5), ItemTypeId(2), 30);
    let result = consume_items(&mut world, (5, 5), ItemTypeId(1), 2);
    assert!(result.satisfied);
    assert_eq!(
        world.get::<GroundItem>(battery).map(|item| item.charges),
        Some(30),
        "the batteries were not asked for"
    );
}

#[test]
fn an_empty_square_gives_nothing() {
    let mut world = World::new();
    let result = consume_items(&mut world, (5, 5), ItemTypeId(1), 1);
    assert_eq!(
        result,
        Consumed {
            taken: 0,
            satisfied: false
        }
    );
}

#[test]
fn several_piles_of_the_same_kind_are_taken_in_the_order_they_lie() {
    let mut world = World::new();
    let first = drop_item(&mut world, (5, 5), ItemTypeId(1), 2);
    drop_item(&mut world, (5, 5), ItemTypeId(1), 5);
    let result = consume_items(&mut world, (5, 5), ItemTypeId(1), 4);
    assert_eq!(
        result,
        Consumed {
            taken: 4,
            satisfied: true
        }
    );
    assert!(
        world.get::<GroundItem>(first).is_none(),
        "the earlier pile went first"
    );
    assert_eq!(
        items_at(&mut world, (5, 5)).len(),
        1,
        "and the later one is short"
    );
}

#[test]
fn asking_for_nothing_takes_nothing_and_is_satisfied() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 5);
    let result = consume_items(&mut world, (5, 5), ItemTypeId(1), 0);
    assert_eq!(
        result,
        Consumed {
            taken: 0,
            satisfied: true
        }
    );
    assert_eq!(
        items_at(&mut world, (5, 5)).len(),
        1,
        "nothing was disturbed"
    );
}
