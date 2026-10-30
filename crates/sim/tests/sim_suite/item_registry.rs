//! The store can say which item is which.
//!
//! Reference: `item_controller`'s `itype_id` table (`item_factory.cpp`), which numbers every item type and answers in
//! both directions. Until this existed the simulation could not answer "what is in my hands" without being handed a
//! lookup, which is a second source of truth wearing a function's clothes.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::item_registry::{self, ItemNames};
use cdda_sim::{ItemTypeId, crafting};



#[test]
fn a_kind_can_be_looked_up_by_number_and_by_name() {
    let mut world = World::new();
    item_registry::set_item_types(&mut world, &["rag", "rope", "rock"]);

    assert_eq!(item_registry::count(&mut world), 3, "three kinds, not counting the null item");
    assert_eq!(item_registry::id_of(&mut world, "rope"), Some(ItemTypeId(2)));
    assert_eq!(item_registry::name_of(&mut world, ItemTypeId(2)).as_deref(), Some("rope"));
    assert_eq!(
        item_registry::name_of(&mut world, ItemTypeId(0)),
        None,
        "the null item has no name, which is what makes it nothing"
    );
    assert_eq!(item_registry::id_of(&mut world, "nothing_of_the_kind"), None);
    assert_eq!(item_registry::name_of(&mut world, ItemTypeId(99)), None, "and no number past the end");
}

#[test]
fn installing_a_second_registry_replaces_the_first() {
    let mut world = World::new();
    item_registry::set_item_types(&mut world, &["rag"]);
    item_registry::set_item_types(&mut world, &["rope"]);
    assert_eq!(item_registry::id_of(&mut world, "rag"), None, "a second load is not a merge");
    assert_eq!(item_registry::id_of(&mut world, "rope"), Some(ItemTypeId(1)));
}

#[test]
fn a_craft_can_be_finished_without_a_caller_supplying_the_item_table() {
    let mut world = World::new();
    item_registry::set_item_types(&mut world, &["rag", "rope"]);

    let crafter = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let mut recipe = cdda_content::recipes::Recipe {
        id: "tie_a_rope".to_owned(),
        result: "rope".to_owned(),
        blacklisted: false,
        components: vec![vec![cdda_content::recipes::Component {
            item: "rag".to_owned(),
            count: 2,
        }]],
        tools: Vec::new(),
        qualities: Vec::new(),
        skill: String::new(),
        difficulty: 0,
        time_moves: 100,
    };
    recipe.result = "rope".to_owned();
    for _ in 0..2 {
        let rag = cdda_sim::drop_item(&mut world, (0, 0), ItemTypeId(1), 1);
        world.entity_mut(rag).remove::<cdda_sim::Position>();
        world
            .entity_mut(rag)
            .insert(cdda_sim::creature::Carried(crafter));
    }

    let names: ItemNames = item_registry::snapshot(&mut world);
    assert_eq!(
        crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        crafting::Craft::Started
    );
    assert_eq!(
        crafting::finish_craft_in_store(&mut world, crafter).as_deref(),
        Some("rope"),
        "the store knew what a rag was and what a rope is, so nothing had to be passed in"
    );
    let carried = cdda_sim::inventory::carried_by_id(&mut world, crafter, |kind| {
        names.name_of(kind)
    });
    assert_eq!(carried.get("rope"), Some(&1));
    assert_eq!(carried.get("rag"), None);
}
