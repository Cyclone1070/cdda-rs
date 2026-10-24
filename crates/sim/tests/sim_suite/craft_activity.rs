//! Starting, pursuing and cancelling a craft.
//!
//! Transcribed from `craft_activity_actor` (`src/activity_actor.cpp:4163-4335`): the unfinished craft is an
//! item that holds its components, so cancelling loses nothing.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    CraftEnd, CraftInProgress, GroundItem, ItemTypeId, cancel_craft, consume_items, contents,
    craft_turn_on, drop_item, items_at, start_craft,
};

fn parts() -> Vec<Vec<(ItemTypeId, i32)>> {
    vec![vec![(ItemTypeId(1), 2)]]
}

#[test]
fn starting_a_craft_takes_the_parts_into_the_unfinished_item() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 5);
    let craft = start_craft(&mut world, (5, 5), &parts(), 1, ItemTypeId(9), 200.0)
        .expect("the parts are there");
    assert!(world.get::<CraftInProgress>(craft).is_some());
    assert_eq!(
        contents(&mut world, craft).len(),
        1,
        "the parts are inside the unfinished craft"
    );
    // **Five, not three.** The two charges the craft took are inside the unfinished craft, and the reference's `item::use_charges` walks an item's **contents**
    // (`visit_items`, `src/item.cpp:13140-13165`), so a caller asking the square for five finds the loose three and then the two in the craft. Asking for three would
    // find only the loose ones and leave the craft untouched.
    assert_eq!(
        consume_items(&mut world, (5, 5), ItemTypeId(1), 3).taken,
        3,
        "the loose three, and the craft's own two are left alone"
    );
    assert_eq!(
        consume_items(&mut world, (5, 5), ItemTypeId(1), 5).taken,
        2,
        "and what is left of the square is what went inside the craft"
    );
}

#[test]
fn working_the_turns_produces_the_result() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 2);
    let craft =
        start_craft(&mut world, (5, 5), &parts(), 1, ItemTypeId(9), 200.0).expect("started");
    assert_eq!(
        craft_turn_on(&mut world, craft, 100, 1.0),
        CraftEnd::Working
    );
    let CraftEnd::Made(result) = craft_turn_on(&mut world, craft, 100, 1.0) else {
        panic!("the second turn finishes it");
    };
    assert_eq!(
        world.get::<GroundItem>(result).map(|item| item.kind),
        Some(ItemTypeId(9))
    );
    assert!(
        world.get::<CraftInProgress>(craft).is_none(),
        "the unfinished craft is gone"
    );
}

#[test]
fn cancelling_keeps_the_parts_inside_the_unfinished_craft() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 2);
    let craft =
        start_craft(&mut world, (5, 5), &parts(), 1, ItemTypeId(9), 200.0).expect("started");
    cancel_craft(&mut world, craft);
    let held = contents(&mut world, craft);
    assert_eq!(held.len(), 1, "the parts are still held");
    let charges: i32 = held
        .iter()
        .filter_map(|entity| world.get::<GroundItem>(*entity).map(|item| item.charges))
        .sum();
    assert_eq!(charges, 2, "both of them, not one");
    assert!(
        items_at(&mut world, (5, 5)).into_iter().all(|entity| world
            .get::<GroundItem>(entity)
            .map(|item| item.kind)
            != Some(ItemTypeId(9))),
        "and no result appeared"
    );
}

#[test]
fn an_interrupted_craft_keeps_its_progress() {
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 2);
    let craft =
        start_craft(&mut world, (5, 5), &parts(), 1, ItemTypeId(9), 300.0).expect("started");
    assert_eq!(
        craft_turn_on(&mut world, craft, 100, 1.0),
        CraftEnd::Working
    );
    // The crafter walks away for a while. Nothing else touches the craft, and progress is not lost.
    let after_first = world
        .get::<CraftInProgress>(craft)
        .map_or(0.0, |component| component.work.fraction());
    assert!((after_first - 1.0 / 3.0).abs() < 1e-9);
    assert_eq!(
        craft_turn_on(&mut world, craft, 100, 1.0),
        CraftEnd::Working
    );
    assert!(matches!(
        craft_turn_on(&mut world, craft, 100, 1.0),
        CraftEnd::Made(_)
    ));
}
