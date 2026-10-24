//! What an item weighs, contents included.
//!
//! Transcribed from `item::weight` (`src/item.cpp:7229-7300`).

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::{Container, GroundItem, ItemTypeId, drop_item, insert_into, item_weight};

fn grams(kind: ItemTypeId) -> cdda_sim::TypeWeight {
    // Type seven stands for the thirty-two `NO_DROP` types in the data; the loader hands the flag over the same way.
    if kind.0 == 7 {
        return cdda_sim::TypeWeight::hidden();
    }
    cdda_sim::TypeWeight::plain(match kind.0 {
        1 => 100, // a rock
        2 => 5,   // a battery, per charge
        9 => 500, // a box
        _ => 0,
    })
}

fn box_holding(world: &mut World, contents: &[ItemTypeId]) -> Entity {
    let box_entity = drop_item(world, (5, 5), ItemTypeId(9), 1);
    world.entity_mut(box_entity).insert(Container {
        capacity_ml: 10_000,
    });
    for kind in contents {
        let item = drop_item(world, (5, 5), *kind, 1);
        insert_into(world, box_entity, item, |_| 1).expect("it fits");
    }
    box_entity
}

#[test]
fn a_plain_item_weighs_what_its_prototype_says() {
    let mut world = World::new();
    let rock = drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    assert_eq!(item_weight(&mut world, rock, grams), 100);
}

#[test]
fn a_stack_weighs_its_charges_times_the_prototype_weight() {
    let mut world = World::new();
    let cells = drop_item(&mut world, (5, 5), ItemTypeId(2), 40);
    assert_eq!(
        item_weight(&mut world, cells, grams),
        200,
        "forty cells at five grams"
    );
}

#[test]
fn what_is_inside_counts() {
    let mut world = World::new();
    let box_entity = box_holding(&mut world, &[ItemTypeId(1), ItemTypeId(1)]);
    assert_eq!(
        item_weight(&mut world, box_entity, grams),
        700,
        "500 of box and 200 of rock"
    );
}

#[test]
fn the_counting_goes_all_the_way_down() {
    let mut world = World::new();
    let inner = box_holding(&mut world, &[ItemTypeId(1)]);
    let outer = drop_item(&mut world, (5, 5), ItemTypeId(9), 1);
    world.entity_mut(outer).insert(Container {
        capacity_ml: 10_000,
    });
    insert_into(&mut world, outer, inner, |_| 1).expect("the inner box fits");
    assert_eq!(
        item_weight(&mut world, outer, grams),
        1100,
        "box, box, rock"
    );
}

#[test]
fn something_that_is_not_there_weighs_nothing() {
    let mut world = World::new();
    let rock = drop_item(&mut world, (5, 5), ItemTypeId(1), 1);
    world.despawn(rock);
    assert_eq!(item_weight(&mut world, rock, grams), 0);
}

#[test]
fn a_stack_of_zero_counts_as_one_rather_than_weighing_nothing() {
    // `item::weight` asks the prototype, and a zero-charge stack is still an object on the floor.
    let mut world = World::new();
    let empty = drop_item(&mut world, (5, 5), ItemTypeId(2), 0);
    assert_eq!(item_weight(&mut world, empty, grams), 5);
}

#[test]
fn charges_that_would_overflow_saturate_instead_of_wrapping() {
    let mut world = World::new();
    let huge = drop_item(&mut world, (5, 5), ItemTypeId(2), i32::MAX);
    assert_eq!(
        item_weight(&mut world, huge, grams),
        i64::from(i32::MAX) * 5
    );
    if let Some(mut item) = world.get_mut::<GroundItem>(huge) {
        item.charges = i32::MAX;
    }
    assert!(
        item_weight(&mut world, huge, |_| cdda_sim::TypeWeight::plain(i64::MAX)) > 0,
        "no wrap to a negative"
    );
}

#[test]
fn an_item_that_does_not_drop_weighs_nothing_contents_included() {
    // **The first branch of `item::weight`** (`src/item.cpp:7235-7239`): a `NO_DROP` item is not really there - the reference's own words are "items just for ease of
    // implementation" - so it answers zero **before** anything else is asked, contents and all. Thirty-two types in the data carry the flag.
    //
    // The order is the rule: a port that weighed the contents and then zeroed the base would leave a `NO_DROP` box holding a rock at a hundred grams, and one that zeroed
    // the whole tree afterwards would be right by accident. The test gives the box contents precisely so the two differ.
    let mut world = World::new();
    let box_entity = box_holding(&mut world, &[ItemTypeId(1)]);
    // The same box, first as an ordinary container: five hundred grams of box and a hundred of rock.
    assert_eq!(item_weight(&mut world, box_entity, grams), 600);
    // Then as a `NO_DROP` one: nothing at all, contents included.
    let ghost = drop_item(&mut world, (6, 6), ItemTypeId(7), 1);
    world.entity_mut(ghost).insert(Container {
        capacity_ml: 10_000,
    });
    let rock = drop_item(&mut world, (6, 6), ItemTypeId(1), 1);
    insert_into(&mut world, ghost, rock, |_| 1).expect("it fits");
    assert_eq!(
        item_weight(&mut world, ghost, grams),
        0,
        "a NO_DROP container weighs nothing, and neither does what is inside it"
    );
}
