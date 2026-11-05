//! A push-items field throwing rocks at its neighbours.
//!
//! Transcribed from `field_processor_fd_push_items` (`src/map_field.cpp:790-836`): rocks only, at
//! least a turn old, thrown to one of the neighbouring squares that also holds the field, losing
//! their age on landing.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::field::FieldEntry;
use cdda_core_types::rng::Rolls;
use cdda_sim::{
    Field, GroundItem, ItemTypeId, PUSH_ITEMS, Position, drop_item_aged, process_push_items,
};

const ROCK: ItemTypeId = ItemTypeId(7);

fn push_field(world: &mut World, at: (i32, i32)) {
    let mut entry = FieldEntry::new(1, 100, false, false);
    entry.set_intensity(1);
    world.spawn((
        Position {
            x: at.0,
            y: at.1,
            z: 0,
        },
        Field {
            id: PUSH_ITEMS,
            entry,
            priority: 0,
        },
    ));
}

#[test]
fn an_aged_rock_is_thrown_to_a_neighbour_that_holds_the_field() {
    let mut world = World::new();
    push_field(&mut world, (5, 5));
    push_field(&mut world, (6, 5));
    drop_item_aged(&mut world, (5, 5), ROCK, 1, 3);
    let mut rolls = Rolls::new(1);
    assert_eq!(process_push_items(&mut world, (5, 5), ROCK, &mut rolls), 1);
    assert!(
        cdda_sim::items_at(&mut world, (5, 5)).is_empty(),
        "the rock left"
    );
    let landed = cdda_sim::items_at(&mut world, (6, 5));
    assert_eq!(landed.len(), 1, "and arrived next door");
    let only = landed.first().copied().expect("a landed rock");
    let item = world.get::<GroundItem>(only).expect("an item");
    assert_eq!((item.kind, item.age), (ROCK, 0), "age reset on landing");
}

#[test]
fn a_fresh_rock_stays_where_it_is_and_draws_nothing() {
    let mut world = World::new();
    push_field(&mut world, (5, 5));
    push_field(&mut world, (6, 5));
    drop_item_aged(&mut world, (5, 5), ROCK, 1, 0);
    let mut rolls = Rolls::new(1);
    let mut control = Rolls::new(1);
    assert_eq!(process_push_items(&mut world, (5, 5), ROCK, &mut rolls), 0);
    assert_eq!(
        rolls.rng(1, 1000),
        control.rng(1, 1000),
        "no dice for a rock that has just landed"
    );
}

#[test]
fn only_rocks_move() {
    let mut world = World::new();
    push_field(&mut world, (5, 5));
    push_field(&mut world, (6, 5));
    drop_item_aged(&mut world, (5, 5), ItemTypeId(99), 1, 5);
    let mut rolls = Rolls::new(1);
    assert_eq!(process_push_items(&mut world, (5, 5), ROCK, &mut rolls), 0);
    assert_eq!(
        cdda_sim::items_at(&mut world, (5, 5)).len(),
        1,
        "the other thing stays"
    );
}

#[test]
fn a_square_with_no_field_next_door_keeps_its_rocks() {
    let mut world = World::new();
    push_field(&mut world, (5, 5));
    drop_item_aged(&mut world, (5, 5), ROCK, 1, 9);
    let mut rolls = Rolls::new(1);
    assert_eq!(process_push_items(&mut world, (5, 5), ROCK, &mut rolls), 0);
    assert_eq!(
        cdda_sim::items_at(&mut world, (5, 5)).len(),
        1,
        "nowhere to throw it"
    );
}

#[test]
fn two_rocks_move_one_after_another_and_keep_their_charges() {
    let mut world = World::new();
    push_field(&mut world, (5, 5));
    push_field(&mut world, (6, 5));
    push_field(&mut world, (4, 5));
    drop_item_aged(&mut world, (5, 5), ROCK, 4, 2);
    drop_item_aged(&mut world, (5, 5), ROCK, 9, 2);
    let mut rolls = Rolls::new(4);
    assert_eq!(process_push_items(&mut world, (5, 5), ROCK, &mut rolls), 2);
    let mut landed = Vec::new();
    for square in [(6, 5), (4, 5)] {
        for entity in cdda_sim::items_at(&mut world, square) {
            if let Some(item) = world.get::<GroundItem>(entity) {
                landed.push(item.charges);
            }
        }
    }
    landed.sort_unstable();
    assert_eq!(landed, vec![4, 9], "both rocks arrived, charges intact");
}
