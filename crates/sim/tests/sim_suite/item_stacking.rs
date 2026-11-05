//! When two items on the ground become one stack.
//!
//! Transcribed from `item::stacks_with` (`src/item.cpp:1693-1760`), keeping the comparisons the
//! port's ground items can actually make.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_sim::{GroundItem, ItemTypeId, StackFacts, drop_and_merge, items_at, stacks_with};

fn rocks(charges: i32) -> StackFacts {
    StackFacts {
        kind: ItemTypeId(1),
        charges,
        burnt: 0,
        by_charges: true,
        damage_level: 0,
    }
}

fn batteries(charges: i32) -> StackFacts {
    StackFacts {
        kind: ItemTypeId(2),
        charges,
        burnt: 0,
        by_charges: false,
        damage_level: 0,
    }
}

#[test]
fn two_piles_of_the_same_charge_counted_item_stack_whatever_their_size() {
    assert!(stacks_with(&rocks(1), &rocks(50)));
}

#[test]
fn two_objects_with_different_charge_levels_do_not_stack() {
    assert!(
        !stacks_with(&batteries(40), &batteries(20)),
        "half-used cells stay apart"
    );
    assert!(
        stacks_with(&batteries(40), &batteries(40)),
        "two full ones do stack"
    );
}

#[test]
fn a_burnt_item_never_stacks_with_a_fresh_one() {
    let mut charred = rocks(3);
    charred.burnt = 1;
    assert!(!stacks_with(&charred, &rocks(3)));
}

#[test]
fn different_kinds_never_stack() {
    assert!(!stacks_with(&rocks(5), &batteries(5)));
}

#[test]
fn dropping_onto_a_matching_stack_adds_to_it_rather_than_making_a_second() {
    let mut world = World::new();
    let first = drop_and_merge(&mut world, (5, 5), rocks(3));
    let second = drop_and_merge(&mut world, (5, 5), rocks(4));
    assert_eq!(first, second, "the same stack took both");
    assert_eq!(
        world.get::<GroundItem>(first).map(|item| item.charges),
        Some(7),
        "and holds seven"
    );
    assert_eq!(items_at(&mut world, (5, 5)).len(), 1, "one pile, not two");
}

#[test]
fn dropping_something_that_does_not_match_makes_its_own_pile() {
    let mut world = World::new();
    drop_and_merge(&mut world, (5, 5), rocks(3));
    drop_and_merge(&mut world, (5, 5), batteries(1));
    assert_eq!(
        items_at(&mut world, (5, 5)).len(),
        2,
        "a rock pile and a battery"
    );
}

#[test]
fn a_dented_item_never_stacks_with_a_sound_one() {
    let mut dented = rocks(3);
    dented.damage_level = 2;
    assert!(
        !stacks_with(&dented, &rocks(3)),
        "the reference compares damage levels"
    );
}
