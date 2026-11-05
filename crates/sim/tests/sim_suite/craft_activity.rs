//! Starting, pursuing and cancelling a craft.
//!
//! Transcribed from `craft_activity_actor` (`src/activity_actor.cpp:4163-4335`): the unfinished craft is an
//! item that holds its components, so cancelling loses nothing.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    CraftEnd, CraftInProgress, GroundItem, ItemTypeId, TypeVolume, TypeWeight, UNFINISHED_CRAFT,
    cancel_craft, consume_items, contents, craft_turn_on, drop_item, item_volume, item_weight,
    items_at, start_craft,
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

#[test]
fn an_unfinished_craft_is_as_big_and_as_heavy_as_the_parts_it_holds() {
    // **The craft's own type says nothing.** Reference: `item::volume`'s craft branch (`src/item.cpp:7522-7534`) adds up the components and floors the answer at one
    // millilitre, and `item::weight`'s (`src/item.cpp:7240-7251`) adds up the same components with no floor at all. The unfinished craft carries the **result's** item
    // type in the reference, so its own volume would be the finished thing's - which is why the rule ignores it.
    let mut world = World::new();
    drop_item(&mut world, (5, 5), ItemTypeId(1), 2);
    let craft = start_craft(&mut world, (5, 5), &parts(), 1, ItemTypeId(9), 200.0)
        .expect("the parts are there");
    // The parts are charge-counted so that the two the craft took are two charges of one entity, which is how the port carries a stack: the reference holds two
    // items and adds both, and a charge-counted type makes those the same number.
    let volume_of = |kind: ItemTypeId| TypeVolume {
        millilitres: if kind == UNFINISHED_CRAFT { 999_999 } else { 100 },
        integral_millilitres: if kind == UNFINISHED_CRAFT { 999_999 } else { 100 },
        count_by_charges: true,
        stack_size: 1,
    };
    let weight_of = |kind: ItemTypeId| TypeWeight {
        milligrams: if kind == UNFINISHED_CRAFT { 999_999_000 } else { 50_000 },
        integral_milligrams: if kind == UNFINISHED_CRAFT { 999_999_000 } else { 50_000 },
        count_by_charges: true,
        no_drop: false,
    };
    assert_eq!(
        item_volume(&mut world, craft, volume_of),
        200,
        "two 100 ml parts, and not the unfinished type's own volume"
    );
    assert_eq!(
        item_weight(&mut world, craft, weight_of),
        100,
        "two 50 g parts, in grams"
    );

    // An empty craft - a recipe of nothing but practices - is still one millilitre, because the reference floors it there to keep zero-volume items out of the world.
    let empty = start_craft(&mut world, (6, 6), &[], 1, ItemTypeId(9), 200.0).expect("no parts");
    assert_eq!(
        item_volume(&mut world, empty, volume_of),
        1,
        "a craft with nothing in it is one millilitre, not zero"
    );
    assert_eq!(
        item_weight(&mut world, empty, weight_of),
        0,
        "and it weighs nothing, because the weight branch has no floor"
    );
}
