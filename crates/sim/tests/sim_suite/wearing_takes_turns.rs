//! Putting clothes on as something that takes turns.
//!
//! Reference: `inventory_ui.cpp:4355` assigns `wear_activity_actor`, whose `do_turn` (`activity_actor.cpp:7001-7035`)
//! puts the garment on once its time is spent. The port had `wear` charging its cost and changing the store in the same
//! breath, which is right for an instant act and wrong for one the turn loop has to be able to interrupt.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_sim::creature::{
    Basis, Began, Doing, Garment, Layering, Wore, begin_wear, set_speed, spawn_creature, wear, wear_cost,
};

#[test]
fn a_garment_that_can_be_paid_for_goes_on_at_once() {
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::creature::give_moves(&mut world, survivor, 1_000);
    let shirt = world.spawn(Garment { layers: Vec::new() }).id();

    assert_eq!(
        begin_wear(&mut world, survivor, shirt, 1_000, Layering::Normal),
        Began::Done,
        "a hundred and fifty moves is inside the thousand it has"
    );
    assert_eq!(
        world.get::<cdda_sim::creature::WornBy>(shirt).map(|worn| worn.0),
        Some(survivor)
    );
    assert!(world.get::<Doing>(survivor).is_none(), "and nothing was queued");
}

#[test]
fn a_garment_that_needs_more_time_becomes_an_activity() {
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::creature::give_moves(&mut world, survivor, 10);
    let coat = world.spawn(Garment { layers: Vec::new() }).id();

    assert_eq!(
        begin_wear(&mut world, survivor, coat, 1_000, Layering::Normal),
        Began::Started
    );
    let doing = world.get::<Doing>(survivor).expect("it is now an activity");
    assert_eq!(doing.activity, "wear");
    assert_eq!(doing.moves_left, wear_cost(1_000, Layering::Normal));
    assert_eq!(doing.basis, Basis::Time, "getting dressed is not a sprint");
    assert_eq!(doing.subject, Some(coat), "and it knows which garment");
    assert!(
        world.get::<cdda_sim::creature::WornBy>(coat).is_none(),
        "the coat is not on yet - that is the point of the activity"
    );
}

#[test]
fn the_turn_loop_puts_the_garment_on_when_the_time_is_up() {
    use cdda_core_types::level::Level;
    use cdda_sim::terrain_table::TerrainTable;
    use cdda_sim::{Prototypes, singletons};

    let mut world = World::new();
    let mut level = Level::new();
    for x in 0..4 {
        level.set_terrain(x, 0, 1, 1);
    }
    cdda_sim::load_level(&mut world, &level);
    let mut terrain = TerrainTable::new();
    terrain.insert(1, true);
    terrain.insert_move_cost(1, 2);
    cdda_sim::set_prototypes(
        &mut world,
        Prototypes {
            terrain,
            furniture: TerrainTable::new(),
        },
    );
    singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(1));

    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    set_speed(&mut world, survivor, 100, 0);
    let shirt = world.spawn(Garment { layers: Vec::new() }).id();
    // Two hundred moves is two turns at a hundred a turn.
    world.entity_mut(survivor).insert(Doing {
        moves_total: 0,
        activity: "wear".to_owned(),
        moves_left: 200,
        basis: Basis::Time,
        subject: Some(shirt),
    });

    cdda_sim::turn::run_turn(&mut world);
    assert!(
        world.get::<cdda_sim::creature::WornBy>(shirt).is_none(),
        "one turn is not two"
    );
    cdda_sim::turn::run_turn(&mut world);
    assert_eq!(
        world.get::<cdda_sim::creature::WornBy>(shirt).map(|worn| worn.0),
        Some(survivor),
        "and the second finishes what the first began"
    );
    assert!(world.get::<Doing>(survivor).is_none());
}

#[test]
fn wearing_a_garment_you_already_have_on_is_still_refused() {
    let mut world = World::new();
    let survivor = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    cdda_sim::creature::give_moves(&mut world, survivor, 1_000);
    let shirt = world.spawn(Garment { layers: Vec::new() }).id();
    assert_eq!(wear(&mut world, survivor, shirt), Wore::Worn);
    assert_eq!(
        begin_wear(&mut world, survivor, shirt, 1_000, Layering::Normal),
        Began::AlreadyCarried,
        "there is no second shirt to put on"
    );
}
