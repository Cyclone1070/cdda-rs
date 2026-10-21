//! Putting things into things.
//!
//! The reference's pockets each have volume, weight and length limits (`item::can_contain`); the port
//! keeps one capacity per container, which is the part every caller needs first.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::{Container, InsertError, ItemTypeId, contents, drop_item, insert_into, take_out};

/// Every test item is 100 ml.
fn volume_of(_item: Entity) -> i64 {
    100
}

fn box_of(world: &mut World, capacity_ml: i64) -> Entity {
    let item = drop_item(world, (5, 5), ItemTypeId(9), 1);
    world.entity_mut(item).insert(Container { capacity_ml });
    item
}

#[test]
fn a_box_takes_what_fits_and_says_so() {
    let mut world = World::new();
    let box_entity = box_of(&mut world, 250);
    let rock = drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    assert_eq!(insert_into(&mut world, box_entity, rock, volume_of), Ok(()));
    assert_eq!(contents(&mut world, box_entity), vec![rock]);
}

#[test]
fn a_full_box_refuses_and_says_how_much_room_is_left() {
    let mut world = World::new();
    let box_entity = box_of(&mut world, 250);
    for _ in 0..2 {
        let item = drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
        insert_into(&mut world, box_entity, item, volume_of).expect("two fit");
    }
    let third = drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    let refused = insert_into(&mut world, box_entity, third, volume_of);
    assert_eq!(
        refused,
        Err(InsertError::TooBig {
            remaining_ml: 50,
            wanted_ml: 100
        })
    );
    assert!(
        contents(&mut world, box_entity).len() == 2,
        "the third stayed out"
    );
}

#[test]
fn taking_something_out_frees_the_room_again() {
    let mut world = World::new();
    let box_entity = box_of(&mut world, 100);
    let first = drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    insert_into(&mut world, box_entity, first, volume_of).expect("one fits exactly");
    take_out(&mut world, first);
    assert!(contents(&mut world, box_entity).is_empty());
    let second = drop_item(&mut world, (5, 5), ItemTypeId(2), 1);
    assert_eq!(
        insert_into(&mut world, box_entity, second, volume_of),
        Ok(())
    );
}

#[test]
fn an_item_cannot_be_in_two_places_at_once() {
    let mut world = World::new();
    let first = box_of(&mut world, 1000);
    let second = box_of(&mut world, 1000);
    let rock = drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    insert_into(&mut world, first, rock, volume_of).expect("goes in the first box");
    assert_eq!(
        insert_into(&mut world, second, rock, volume_of),
        Err(InsertError::AlreadyInside)
    );
}

#[test]
fn something_that_is_not_a_container_takes_nothing() {
    let mut world = World::new();
    let rock = drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    let other = drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    assert_eq!(
        insert_into(&mut world, rock, other, volume_of),
        Err(InsertError::TooBig {
            remaining_ml: 0,
            wanted_ml: 100
        })
    );
}
