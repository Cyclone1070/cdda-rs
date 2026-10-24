//! A liquid on the floor cannot be used unless the floor holds liquid, and the pile is not even offered to the filter.
//!
//! Reference: `item_stack::use_charges` (`src/item_stack.cpp:146-160`). The walk only hands an item to `item::use_charges` when it is not a liquid **or** the square carries
//! `TFLAG_LIQUIDCONT`: `( !a->made_of( phase_id::LIQUID ) || ( ... && has_flag( TFLAG_LIQUIDCONT, pos ) ) ) && a->use_charges( ... )`. A puddle on a plain floor is
//! therefore not a source at all - it is scenery - and the gate is asked **before** the filter, so a predicate never sees it.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_sim::{GroundItem, ItemTypeId, Position, consume_items_held, drop_item};

const CLEAN_WATER: ItemTypeId = ItemTypeId(1);
const ROCK: ItemTypeId = ItemTypeId(2);

fn charges(world: &mut World) -> Vec<i32> {
    let mut query = world.query::<(&Position, &GroundItem)>();
    query
        .iter(world)
        .filter(|(at, _)| (at.x, at.y) == (3, 3))
        .map(|(_, item)| item.charges)
        .collect()
}

#[test]
fn a_liquid_on_a_plain_floor_cannot_be_used() {
    let mut world = World::new();
    drop_item(&mut world, (3, 3), CLEAN_WATER, 8);
    let taken = consume_items_held(
        &mut world,
        (3, 3),
        CLEAN_WATER,
        8,
        |kind| kind == CLEAN_WATER,
        false,
        |_| true,
    );
    assert_eq!(taken.taken, 0, "the puddle is scenery");
    assert!(!taken.satisfied);
    assert_eq!(charges(&mut world), vec![8]);
}

#[test]
fn the_same_liquid_on_a_container_square_can_be_used() {
    let mut world = World::new();
    drop_item(&mut world, (3, 3), CLEAN_WATER, 8);
    let taken = consume_items_held(
        &mut world,
        (3, 3),
        CLEAN_WATER,
        8,
        |kind| kind == CLEAN_WATER,
        true,
        |_| true,
    );
    assert_eq!(taken.taken, 8);
    assert!(taken.satisfied);
    assert!(charges(&mut world).is_empty(), "the puddle was emptied");
}

#[test]
fn a_solid_pile_beside_a_puddle_is_still_taken() {
    let mut world = World::new();
    drop_item(&mut world, (3, 3), CLEAN_WATER, 8);
    drop_item(&mut world, (3, 3), ROCK, 3);
    let taken = consume_items_held(
        &mut world,
        (3, 3),
        ROCK,
        3,
        |kind| kind == CLEAN_WATER,
        false,
        |_| true,
    );
    assert_eq!(taken.taken, 3, "the rocks are not a liquid");
    assert_eq!(charges(&mut world), vec![8], "and the puddle is untouched");
}

/// The gate is asked before the filter, so a predicate never sees a puddle it could not have been given.
#[test]
fn the_filter_is_never_asked_about_a_puddle_on_a_plain_floor() {
    let mut world = World::new();
    drop_item(&mut world, (3, 3), CLEAN_WATER, 8);
    drop_item(&mut world, (3, 3), ROCK, 3);
    let mut seen: Vec<ItemTypeId> = Vec::new();
    let taken = consume_items_held(
        &mut world,
        (3, 3),
        CLEAN_WATER,
        8,
        |kind| kind == CLEAN_WATER,
        false,
        |item| {
            seen.push(item.kind);
            true
        },
    );
    assert_eq!(taken.taken, 0);
    assert_eq!(
        seen,
        vec![ROCK],
        "the filter was shown the rock and never the puddle"
    );
}
