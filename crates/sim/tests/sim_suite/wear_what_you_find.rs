//! The whole errand: something is on the floor, a survivor picks it up, puts it on, and a blow meets the difference.
//!
//! Each rule has its own gate. This one exists because rules that are right on their own can still be wired wrongly
//! to each other - a pickup that leaves a ghost in the index, a garment worn while still on the square, an armour
//! that was derived before the vest was in hand. The reference has no single function for this either; it is what a
//! player does with three keys, and it is the first thing anyone will try in the port.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_core_types::level::Level;
use cdda_sim::creature::{
    Garment, Picked, Wore, armour_on, give_moves, pick_up, spawn_creature, wear,
};
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

fn a_vest_on_the_floor(world: &mut World) -> Entity {
    let vest = cdda_sim::drop_item(world, (1, 0), cdda_sim::ItemTypeId(7), 1);
    world.entity_mut(vest).insert(Garment {
        layers: vec![(
            "torso".to_owned(),
            cdda_content::armour::Layer {
                damage_type: "bash".to_owned(),
                resist: 6.0,
                thickness: 0.5,
                cover: 100,
            },
        )],
        vitals: Vec::new(),
    });
    vest
}

#[test]
fn a_vest_on_the_floor_can_be_picked_up_worn_and_can_take_a_blow() {
    let mut world = world_with_floor();
    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    give_moves(&mut world, survivor, 500);
    let vest = a_vest_on_the_floor(&mut world);
    assert_eq!(armour_on(&world, survivor, "torso", None), 0, "nothing on yet");

    // Wearing it where it lies is not a thing: it has to be in hand first, which is the order a player does it in,
    // and the order `Character::wear` assumes - it takes an inventory item, not a map one.
    assert_eq!(
        wear(&mut world, survivor, vest),
        Wore::NotInHand,
        "a vest on the floor is not a vest you are wearing"
    );
    assert_eq!(armour_on(&world, survivor, "torso", None), 0, "so nothing protects the torso yet");
    assert!(world.get::<Position>(vest).is_some(), "and it is still on the floor");

    assert_eq!(pick_up(&mut world, survivor, vest, 250), Picked::Taken);
    assert_eq!(wear(&mut world, survivor, vest), Wore::Worn);

    assert_eq!(
        armour_on(&world, survivor, "torso", None),
        3,
        "half a millimetre of steel is three points, and it took the whole errand to get there"
    );
    assert!(
        world.get::<Position>(vest).is_none(),
        "the vest is in hand, so it has no square"
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (1, 0)).len(),
        0,
        "and the floor it came off is empty"
    );
}
