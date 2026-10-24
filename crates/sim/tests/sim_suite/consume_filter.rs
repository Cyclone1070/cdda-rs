//! Taking charges off a square with a filter, which is how the reference keeps a pile it may not touch.
//!
//! Reference: `item::use_charges` (`src/item.cpp:13135-13180`). The walk visits each item in the order it lies there and asks the **filter first**: an item the filter
//! rejects is skipped and the walk carries on (`if( !filter( *e ) ) return VisitResponse::NEXT;`), long before the type is compared. So a square holding two piles of the
//! same thing, one of which must be left alone, gives up the other one - and a port that filtered afterwards would have already spent the wrong pile.

#![allow(clippy::expect_used, clippy::panic)]

use bevy_ecs::prelude::*;
use cdda_sim::{GroundItem, ItemTypeId, Position, consume_items_with, drop_item};

const WATER: ItemTypeId = ItemTypeId(1);
const ROCK: ItemTypeId = ItemTypeId(2);

fn world() -> World {
    let mut world = World::new();
    drop_item(&mut world, (3, 3), WATER, 4);
    drop_item(&mut world, (3, 3), WATER, 6);
    world
}

/// How many charges are left on the square, in the order they lie there.
fn charges(world: &mut World) -> Vec<i32> {
    let mut query = world.query::<(&Position, &GroundItem)>();
    query
        .iter(world)
        .filter(|(at, _)| (at.x, at.y) == (3, 3))
        .map(|(_, item)| item.charges)
        .collect()
}

#[test]
fn a_rejected_pile_is_skipped_and_the_next_one_gives_up_its_charges() {
    let mut world = world();
    // Only the second pile - the one holding six - may be touched.
    let taken = consume_items_with(&mut world, (3, 3), WATER, 5, |item| item.charges == 6);
    assert_eq!(taken.taken, 5, "all five came from the pile that was allowed");
    assert!(taken.satisfied);
    assert_eq!(
        charges(&mut world),
        vec![4, 1],
        "the forbidden pile is untouched and the allowed one is one short"
    );
}

#[test]
fn a_filter_that_rejects_everything_takes_nothing() {
    let mut world = world();
    let taken = consume_items_with(&mut world, (3, 3), WATER, 5, |_| false);
    assert_eq!(taken.taken, 0);
    assert!(!taken.satisfied);
    assert_eq!(charges(&mut world), vec![4, 6], "both piles still there");
}

/// Before this round the filter did not exist, so the two piles were taken in plain spawn order.
#[test]
fn without_a_filter_the_piles_are_taken_in_the_order_they_lie_there() {
    let mut world = world();
    let taken = cdda_sim::consume_items(&mut world, (3, 3), WATER, 5);
    assert_eq!(taken.taken, 5);
    assert_eq!(
        charges(&mut world),
        vec![5],
        "the first pile of four was emptied and removed, and one came off the second"
    );
}

#[test]
fn a_pile_that_is_emptied_is_removed_from_the_square() {
    let mut world = world();
    let taken = consume_items_with(&mut world, (3, 3), WATER, 4, |item| item.charges == 4);
    assert_eq!(taken.taken, 4);
    assert_eq!(charges(&mut world), vec![6], "the emptied pile is gone");
}

/// The filter sees **every** pile, including the ones of another kind, and in the order they lie there.
///
/// Reference: `item::use_charges` (`src/item.cpp:13145-13149`): `visit_items` hands each item to the filter before anything compares its type, so the predicate is asked
/// about a pile it could never supply. That is the observable shape of the order - a filter with a side effect (a message, a mark, a counter) sees them all - and it is
/// the difference between filtering first and filtering afterwards.
#[test]
fn the_filter_sees_every_pile_including_the_ones_of_another_kind() {
    let mut world = World::new();
    drop_item(&mut world, (3, 3), WATER, 4);
    drop_item(&mut world, (3, 3), ROCK, 1);
    drop_item(&mut world, (3, 3), WATER, 6);
    let mut seen: Vec<(ItemTypeId, i32)> = Vec::new();
    let taken = consume_items_with(&mut world, (3, 3), WATER, 5, |item| {
        seen.push((item.kind, item.charges));
        item.charges == 6
    });
    assert_eq!(taken.taken, 5);
    assert_eq!(
        seen,
        vec![(WATER, 4), (ROCK, 1), (WATER, 6)],
        "the predicate was shown every pile, in spawn order, including the rock"
    );
}
