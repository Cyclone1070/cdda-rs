//! What an inventory answers about the items in it.
//!
//! Transcribed from `visitable::amount_of` (`src/visitable.cpp:964-978`) and `visitable::charges_of` (`src/visitable.cpp:819-875`), whose walks are the counting half of
//! the same `visit_items` the port already uses for `use_amount` and `use_charges`.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    Container, Inventory, ItemFacts, ItemFlags, ItemTypeId, amount_of, charges_of, drop_item,
    insert_into,
};

const ROCK: ItemTypeId = ItemTypeId(1);
const BOX: ItemTypeId = ItemTypeId(2);
const BATTERY: ItemTypeId = ItemTypeId(3);

fn broken(world: &mut World, item: Entity) {
    world
        .entity_mut(item)
        .insert(ItemFlags(["ITEM_BROKEN".to_owned()].into_iter().collect()));
}

#[test]
fn an_inventory_counts_the_items_in_it_and_what_is_inside_them() {
    let mut world = World::new();
    let mut inventory = Inventory::new();
    for _ in 0..3 {
        let rock = drop_item(&mut world, (1, 1), ROCK, 1);
        inventory.add(rock);
    }
    // **A rock in a box is still a rock in the inventory.** The reference's `visit_items` descends into contents, so `amount_of` sees it.
    let rock = drop_item(&mut world, (1, 1), ROCK, 1);
    let box_entity = drop_item(&mut world, (1, 1), BOX, 1);
    world.entity_mut(box_entity).insert(Container::new(1_000));
    insert_into(&mut world, box_entity, rock, |_| ItemFacts::solid(10)).expect("the box takes it");
    inventory.add(box_entity);

    // **A broken rock is not counted at all.** `amount_of` skips `ITEM_BROKEN` before it looks at the type (`src/visitable.cpp:969-971`).
    let broken_rock = drop_item(&mut world, (1, 1), ROCK, 1);
    broken(&mut world, broken_rock);
    inventory.add(broken_rock);

    assert_eq!(
        amount_of(&mut world, &inventory, Some(ROCK), true, i32::MAX),
        4,
        "three loose, one in the box, and not the broken one"
    );
    assert_eq!(
        amount_of(&mut world, &inventory, Some(ROCK), true, 2),
        2,
        "the limit stops the walk, which is what makes it safe to ask a full inventory 'are there two?'"
    );
    assert_eq!(
        amount_of(&mut world, &inventory, None, true, i32::MAX),
        5,
        "`any` counts every item, including the broken rock and the box"
    );
}

#[test]
fn charges_are_summed_not_the_items_that_hold_them() {
    let mut world = World::new();
    let mut inventory = Inventory::new();
    for charges in [100, 25] {
        let battery = drop_item(&mut world, (1, 1), BATTERY, charges);
        inventory.add(battery);
    }
    // A battery in a box is a battery the inventory can spend, and a broken one cannot be spent.
    let inside = drop_item(&mut world, (1, 1), BATTERY, 7);
    let box_entity = drop_item(&mut world, (1, 1), BOX, 1);
    world.entity_mut(box_entity).insert(Container::new(1_000));
    insert_into(&mut world, box_entity, inside, |_| ItemFacts::solid(10)).expect("the box takes it");
    inventory.add(box_entity);
    let broken_battery = drop_item(&mut world, (1, 1), BATTERY, 40);
    broken(&mut world, broken_battery);
    inventory.add(broken_battery);

    assert_eq!(
        charges_of(&mut world, &inventory, BATTERY, i32::MAX),
        132,
        "100 + 25 + the 7 in the box, and not the broken 40"
    );
    assert_eq!(
        charges_of(&mut world, &inventory, BATTERY, 30),
        30,
        "and the limit stops the count at the limit rather than at an item boundary"
    );
    assert_eq!(
        amount_of(&mut world, &inventory, Some(BATTERY), true, i32::MAX),
        3,
        "while amount_of counts the batteries themselves: two loose and one in the box"
    );
}
