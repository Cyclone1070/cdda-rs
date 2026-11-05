//! What an inventory answers about the items in it.
//!
//! Transcribed from `visitable::amount_of` (`src/visitable.cpp:964-978`) and `visitable::charges_of` (`src/visitable.cpp:819-875`), whose walks are the counting half of
//! the same `visit_items` the port already uses for `use_amount` and `use_charges`.

#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use bevy_ecs::prelude::*;
use cdda_sim::{
    Consumed, Container, Inventory, ItemFacts, ItemFlags, ItemTypeId, amount_of, charges_of,
    contents, drop_item, force_into, insert_into,
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

#[test]
fn an_inventory_spends_charges_from_the_item_before_the_box_and_whole_items_the_other_way() {
    // **Two walks, two orders**, and the reference keeps them apart on purpose (`src/item.cpp:13140-13165` against `src/item.cpp:12868-12905`): spending charges visits an
    // item **before** its contents, so a loose battery is emptied before the one in the box; taking whole items visits contents **first**, bottom up.
    let mut world = World::new();
    let mut inventory = Inventory::new();
    let loose = drop_item(&mut world, (1, 1), BATTERY, 100);
    inventory.add(loose);
    let inside = drop_item(&mut world, (1, 1), BATTERY, 7);
    let box_entity = drop_item(&mut world, (1, 1), BOX, 1);
    world.entity_mut(box_entity).insert(Container::new(1_000));
    insert_into(&mut world, box_entity, inside, |_| ItemFacts::solid(10)).expect("the box takes it");
    inventory.add(box_entity);

    assert!(inventory.has_charges(&mut world, BATTERY, 107), "107 charges are there");
    assert!(!inventory.has_charges(&mut world, BATTERY, 108), "and no more than 107");

    // 100 is exactly the loose battery, so the box is untouched and the request is met.
    let spend = inventory.use_charges(&mut world, BATTERY, 100, |_| true);
    assert_eq!(spend, Consumed { taken: 100, satisfied: true });
    assert_eq!(
        charges_of(&mut world, &inventory, BATTERY, i32::MAX),
        7,
        "the loose battery is empty and gone, the box's seven remain"
    );
    assert_eq!(
        contents(&mut world, box_entity).len(),
        1,
        "an emptied battery leaves the inventory, and the box is not touched"
    );

    // Ten asked of the remaining seven: seven taken, and the caller learns the request came up short.
    let short = inventory.use_charges(&mut world, BATTERY, 10, |_| true);
    assert_eq!(short, Consumed { taken: 7, satisfied: false });

    // And a whole item is a whole item: one battery, not one charge.
    let loose_again = drop_item(&mut world, (1, 1), BATTERY, 50);
    inventory.add(loose_again);
    let whole = inventory.use_amount(&mut world, BATTERY, 1, |_| true);
    assert_eq!(whole, Consumed { taken: 1, satisfied: true });
    assert_eq!(
        charges_of(&mut world, &inventory, BATTERY, i32::MAX),
        0,
        "the fifty charges left with the battery that held them"
    );
}

#[test]
fn the_charge_walk_only_enters_a_container_but_the_count_walk_enters_anything() {
    // **The one gate that separates the two walks.** `charges_of` ends its visit with `e->is_container() ? NEXT : SKIP` (`src/visitable.cpp:853`), so a thing that is not a
    // container supplies only its own charges and never what it happens to hold; `amount_of` has no such gate (`src/visitable.cpp:964-978`). A corpse with a battery
    // forced into it is exactly that case: the battery is **in** the inventory and does **not** count toward its charges.
    let mut world = World::new();
    let mut inventory = Inventory::new();
    let loose = drop_item(&mut world, (1, 1), BATTERY, 100);
    inventory.add(loose);
    let hidden = drop_item(&mut world, (1, 1), BATTERY, 40);
    let corpse = drop_item(&mut world, (1, 1), ROCK, 1);
    force_into(&mut world, corpse, hidden).expect("a corpse holds what was forced into it");
    inventory.add(corpse);

    assert_eq!(
        charges_of(&mut world, &inventory, BATTERY, i32::MAX),
        100,
        "the 40 inside something that is not a container are not spendable charges"
    );
    assert_eq!(
        amount_of(&mut world, &inventory, Some(BATTERY), true, i32::MAX),
        2,
        "but they are still two batteries in the inventory, because the count walk descends into anything"
    );
    // And a request the hidden battery could have met comes up short, because the walk never reaches it.
    let short = inventory.use_charges(&mut world, BATTERY, 140, |_| true);
    assert_eq!(
        short,
        Consumed {
            taken: 100,
            satisfied: false
        },
        "the loose battery gives all 100 and the hidden 40 are never offered"
    );
}
