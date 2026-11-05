//! Taking something off the ground.
//!
//! Reference: `Character::pick_up` moves an item from the map into the character's inventory and charges
//! `item_handling_cost` (`character.cpp:7283-7290`) for the trouble. Two facts have to hold at once and they pull in
//! different directions: the item must leave the square it was on - or the index would claim two surfaces hold it -
//! and it must still be the same item afterwards.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_sim::creature::{Carried, Picked, give_moves, handling_cost, moves_left, pick_up, spawn_creature};
use cdda_sim::terrain_table::TerrainTable;
use cdda_sim::{Position, Prototypes};

const FLOOR: u32 = 1;

fn world_with_floor() -> World {
    let mut world = World::new();
    let mut level = Level::new();
    for x in 0..4 {
        level.set_terrain(x, 0, FLOOR, FLOOR);
    }
    cdda_sim::load_level(&mut world, &level);
    let mut terrain = TerrainTable::new();
    terrain.insert(FLOOR, true);
    terrain.insert_move_cost(FLOOR, 2);
    cdda_sim::set_prototypes(
        &mut world,
        Prototypes {
            terrain,
            furniture: TerrainTable::new(),
        },
    );
    world
}

#[test]
fn picking_something_up_takes_it_off_the_square() {
    let mut world = world_with_floor();
    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    give_moves(&mut world, survivor, 500);
    let rock = cdda_sim::drop_item(&mut world, (1, 0), cdda_sim::ItemTypeId(9), 1);
    let before = cdda_sim::items_at(&mut world, (1, 0));
    assert_eq!(before.len(), 1, "one thing on the square");

    // A rock is 250 ml, so 100 + 12 moves.
    assert_eq!(pick_up(&mut world, survivor, rock, 250), Picked::Taken);

    assert_eq!(
        cdda_sim::items_at(&mut world, (1, 0)).len(),
        0,
        "and nothing on the square any more"
    );
    assert!(
        world.get::<Position>(rock).is_none(),
        "the carried thing has no square, which is what keeps it out of the index"
    );
    assert_eq!(world.get::<Carried>(rock), Some(&Carried(survivor)), "and it knows who holds it");
    assert_eq!(moves_left(&world, survivor), 500 - 112, "having paid to handle it");
}

#[test]
fn a_thing_on_another_square_is_not_in_reach() {
    let mut world = world_with_floor();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    give_moves(&mut world, survivor, 500);
    let rock = cdda_sim::drop_item(&mut world, (2, 0), cdda_sim::ItemTypeId(9), 1);

    assert_eq!(pick_up(&mut world, survivor, rock, 250), Picked::NotHere);
    assert_eq!(cdda_sim::items_at(&mut world, (2, 0)).len(), 1, "it stays where it is");
    assert_eq!(moves_left(&world, survivor), 500, "and nothing was spent");
}

#[test]
fn picking_something_up_twice_is_refused_and_costs_nothing() {
    let mut world = world_with_floor();
    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    give_moves(&mut world, survivor, 500);
    let rock = cdda_sim::drop_item(&mut world, (1, 0), cdda_sim::ItemTypeId(9), 1);
    assert_eq!(pick_up(&mut world, survivor, rock, 250), Picked::Taken);
    let after_first = moves_left(&world, survivor);

    assert_eq!(
        pick_up(&mut world, survivor, rock, 250),
        Picked::AlreadyCarried,
        "it is already in hand"
    );
    assert_eq!(moves_left(&world, survivor), after_first, "and asking again is free");
}

#[test]
fn a_thing_too_big_for_the_moves_left_waits() {
    let mut world = world_with_floor();
    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    give_moves(&mut world, survivor, 50);
    let rock = cdda_sim::drop_item(&mut world, (1, 0), cdda_sim::ItemTypeId(9), 1);

    assert_eq!(pick_up(&mut world, survivor, rock, 250), Picked::TooSlow);
    assert_eq!(cdda_sim::items_at(&mut world, (1, 0)).len(), 1, "it is still on the floor");
    assert_eq!(moves_left(&world, survivor), 50, "and the turn was not spent");
}

#[test]
fn handling_fifty_moves_a_litre_up_to_two_hundred() {
    assert_eq!(handling_cost(0), 100, "a handful of nothing still takes a base turn");
    assert_eq!(handling_cost(1_000), 150, "a litre is fifty moves");
    assert_eq!(handling_cost(2_000), 200, "two litres a hundred");
    assert_eq!(handling_cost(4_000), 300, "four litres the cap");
    assert_eq!(
        handling_cost(400_000),
        300,
        "and four hundred litres is still three hundred: the reference caps the volume term, not the total"
    );
}
