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
    item_registry::set_item_types(&mut world, &[("rag", 250), ("rope", 500), ("rock", 1_000)]);

    assert_eq!(item_registry::count(&mut world), 3, "three kinds, not counting the null item");
    assert_eq!(item_registry::id_of(&mut world, "rope"), Some(ItemTypeId(2)));
    assert_eq!(item_registry::name_of(&mut world, ItemTypeId(2)).as_deref(), Some("rope"));
    assert_eq!(
        item_registry::name_of(&mut world, ItemTypeId(0)),
        None,
        "the null item has no name, which is what makes it nothing"
    );
    assert_eq!(item_registry::id_of(&mut world, "nothing_of_the_kind"), None);
    assert_eq!(
        item_registry::volume_of(&mut world, ItemTypeId(2)),
        500,
        "a rope takes half a litre"
    );
    assert_eq!(
        item_registry::volume_of(&mut world, ItemTypeId(0)),
        0,
        "and the null item takes no room, because it is not a thing"
    );
    assert_eq!(item_registry::name_of(&mut world, ItemTypeId(99)), None, "and no number past the end");
}

#[test]
fn installing_a_second_registry_replaces_the_first() {
    let mut world = World::new();
    item_registry::set_item_types(&mut world, &[("rag", 250)]);
    item_registry::set_item_types(&mut world, &[("rope", 500)]);
    assert_eq!(item_registry::id_of(&mut world, "rag"), None, "a second load is not a merge");
    assert_eq!(item_registry::id_of(&mut world, "rope"), Some(ItemTypeId(1)));
}

#[test]
fn a_craft_can_be_finished_without_a_caller_supplying_the_item_table() {
    let mut world = World::new();
    item_registry::set_item_types(&mut world, &[("rag", 250), ("rope", 500)]);

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

/// Picking something up costs what the store says it is, not what a caller claims.
#[test]
fn a_pickup_reads_the_size_of_what_it_is_lifting() {
    use cdda_core_types::level::Level;
    use cdda_sim::creature::{Picked, give_moves, moves_left, pick_up_in_store, spawn_creature};
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
    // A four-litre thing: 100 + 200 moves, the most the volume term can be.
    item_registry::set_item_types(&mut world, &[("jerrycan", 4_000)]);

    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    give_moves(&mut world, survivor, 500);
    let can = cdda_sim::drop_item(&mut world, (1, 0), ItemTypeId(1), 1);

    assert_eq!(pick_up_in_store(&mut world, survivor, can), Picked::Taken);
    assert_eq!(
        moves_left(&world, survivor),
        500 - 300,
        "a hundred for the handling and two hundred for four litres"
    );
}
