//! Starting a craft.
//!
//! Reference: `Character::start_craft` assigns `ACT_CRAFT` with `recipe::batch_time( guy, batch, 1.0, 0 )` moves, and
//! `Character::making_would_work` refuses when the components are not on hand. The two halves belong together: a craft
//! that started without its components would silently produce something from nothing, and one that started with no
//! duration would finish before it began.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_content::recipes::Recipe;
use cdda_sim::crafting::{Craft, Making, begin_craft};
use cdda_sim::creature::{Basis, Doing, moves_left, set_speed, spawn_creature};

/// A name table with the given numbers filled in, the way a loaded registry would be.
fn names_for(entries: &[(u32, &str)]) -> cdda_sim::item_registry::ItemNames {
    let highest = entries.iter().map(|(number, _)| *number).max().unwrap_or(0);
    let mut names = cdda_sim::item_registry::ItemNames {
        names: vec![String::new(); highest as usize + 1],
        by_name: std::collections::BTreeMap::new(),
        volumes_ml: vec![0; highest as usize + 1],
        weights_g: vec![0; highest as usize + 1],
        holds_ml: vec![0; highest as usize + 1],
        pockets: vec![Vec::new(); highest as usize + 1],
    };
    for (number, name) in entries {
        if let Some(slot) = names.names.get_mut(*number as usize) {
            (*name).clone_into(slot);
        }
        names
            .by_name
            .insert((*name).to_owned(), cdda_sim::ItemTypeId(*number));
    }
    names
}

fn recipe(time_moves: i64, needs: &str, count: i32) -> Recipe {
    Recipe {
        id: "test_recipe".to_owned(),
        result: "thing".to_owned(),
        blacklisted: false,
        components: vec![vec![cdda_content::recipes::Component {
            item: needs.to_owned(),
            count,
        }]],
        tools: Vec::new(),
        qualities: Vec::new(),
        skill: String::new(),
        difficulty: 0,
        time_moves,
    }
}

#[test]
fn a_craft_begins_with_the_time_the_recipe_says() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let recipe = recipe(20_000, "rag", 2);

    assert_eq!(
        begin_craft(&mut world, crafter, &recipe, 1, |item| i32::from(item == "rag") * 2),
        Craft::Started,
        "two rags are on hand and the recipe wants two"
    );

    let doing = world.get::<Doing>(crafter).expect("the craft is now what it is doing");
    assert_eq!(doing.activity, "craft");
    assert_eq!(doing.moves_left, 20_000, "twenty thousand moves, as the recipe says");
    assert_eq!(doing.basis, Basis::Speed, "a craft goes at the crafter's pace");
    assert_eq!(
        world.get::<Making>(crafter).map(|making| making.batch),
        Some(1)
    );
}

#[test]
fn a_batch_multiplies_the_time() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let recipe = recipe(5_000, "rag", 1);

    assert_eq!(
        begin_craft(&mut world, crafter, &recipe, 3, |_| 3),
        Craft::Started
    );
    assert_eq!(
        world.get::<Doing>(crafter).map(|doing| doing.moves_left),
        Some(15_000),
        "three batches is three times the time"
    );
}

#[test]
fn a_craft_without_its_components_does_not_start() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let recipe = recipe(20_000, "rag", 2);

    assert_eq!(
        begin_craft(&mut world, crafter, &recipe, 1, |item| i32::from(item == "rag")),
        Craft::MissingComponents,
        "one rag is not two"
    );
    assert!(world.get::<Doing>(crafter).is_none(), "and nothing was started");
    assert!(world.get::<Making>(crafter).is_none());
}

#[test]
fn a_busy_crafter_cannot_start_something_else() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    world.entity_mut(crafter).insert(Doing {
        activity: "pickup".to_owned(),
        moves_left: 100,
        basis: Basis::Speed,
        subject: None,
    });
    let recipe = recipe(20_000, "rag", 1);
    assert_eq!(begin_craft(&mut world, crafter, &recipe, 1, |_| 99), Craft::Busy);
}

#[test]
fn a_recipe_with_no_time_is_refused_rather_than_finished_instantly() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let recipe = recipe(0, "rag", 1);
    assert_eq!(
        begin_craft(&mut world, crafter, &recipe, 1, |_| 99),
        Craft::NoTimeToMakeIt,
        "no recipe in the data is instant, so a zero is a reader that failed, not a fast recipe"
    );
    assert!(world.get::<Doing>(crafter).is_none());
}

#[test]
fn the_craft_takes_the_turns_it_said_it_would() {
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

    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    set_speed(&mut world, crafter, 100, 0);
    // Three hundred moves is three turns at a hundred a turn.
    let recipe = recipe(300, "rag", 1);
    assert_eq!(begin_craft(&mut world, crafter, &recipe, 1, |_| 1), Craft::Started);

    cdda_sim::turn::run_turn(&mut world);
    cdda_sim::turn::run_turn(&mut world);
    assert!(world.get::<Doing>(crafter).is_some(), "two turns is not three");
    assert!(moves_left(&world, crafter) <= 0, "and each turn was spent on the work");

    cdda_sim::turn::run_turn(&mut world);
    assert!(
        world.get::<Doing>(crafter).is_none(),
        "the third turn finishes what the recipe said would take three"
    );
}

/// A recipe is satisfied by what the creature is really holding.
///
/// The last round's `begin_craft` took the count as a closure, so any caller could answer for any number: a rule about
/// availability that no store ever had to back. This walks the join - items carried as entities, counted by the id the
/// data uses, then handed to the availability rule - and refuses when one is short.
#[test]
fn a_recipe_is_satisfied_by_what_is_in_hand_and_nothing_else() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    // Kind 7 is "rag" in this test's registry; kind 9 is a rock, which no recipe here wants.
    // The names a world's registry would hold, written out: kind 7 is "rag", kind 9 a rock nobody here wants.
    let names = names_for(&[(7, "rag"), (9, "rock")]);
    let recipe = recipe(20_000, "rag", 2);

    // One rag in each hand, as two separate entities.
    for _ in 0..2 {
        let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(7), 1);
        world
            .entity_mut(rag)
            .remove::<cdda_sim::Position>();
        world
            .entity_mut(rag)
            .insert(cdda_sim::creature::Carried(crafter));
    }
    let carried = cdda_sim::inventory::carried_by_id(&mut world, crafter, |kind| names.name_of(kind));
    assert_eq!(carried.get("rag"), Some(&2), "two rags are in hand");

    assert_eq!(
        cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        Craft::Started,
        "and the recipe wants two"
    );

    // A second creature with one rag is short, and the same rule refuses it.
    let other = spawn_creature(&mut world, "another survivor", (1, 0, 0));
    let rag = cdda_sim::drop_item(&mut world, (1, 0), cdda_sim::ItemTypeId(7), 1);
    world.entity_mut(rag).remove::<cdda_sim::Position>();
    world
        .entity_mut(rag)
        .insert(cdda_sim::creature::Carried(other));
    assert_eq!(
        cdda_sim::crafting::begin_craft_from_carried(&mut world, other, &recipe, 1, &names),
        Craft::MissingComponents,
        "one rag is not two"
    );
}

/// Finishing a craft takes what it used and puts what it made in your hands.
///
/// Reference: `activity_handlers::craft_finish` -> `recipe::create_results`. The two halves have to agree: a craft
/// that produced without consuming would fill the world with free goods, and one that consumed without producing
/// would be indistinguishable from a bug until you looked at your hands.
#[test]
fn finishing_a_craft_consumes_the_components_and_makes_the_result() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let names = names_for(&[(7, "rag"), (11, "rope")]);
    // Two rags and a recipe that wants them.
    let mut rags = Vec::new();
    for _ in 0..2 {
        let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(7), 1);
        world.entity_mut(rag).remove::<cdda_sim::Position>();
        world.entity_mut(rag).insert(cdda_sim::creature::Carried(crafter));
        rags.push(rag);
    }
    let mut recipe = recipe(100, "rag", 2);
    recipe.result = "rope".to_owned();
    assert_eq!(
        cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        Craft::Started
    );

    let made = cdda_sim::crafting::finish_craft(&mut world, crafter, &names);

    assert_eq!(made.as_deref(), Some("rope"), "a rope was made");
    assert!(
        rags.iter().all(|rag| world.get_entity(*rag).is_err()),
        "and both rags were used up"
    );
    let carried = cdda_sim::inventory::carried_by_id(&mut world, crafter, |kind| names.name_of(kind));
    assert_eq!(carried.get("rag"), None, "no rags left");
    assert_eq!(carried.get("rope"), Some(&1), "one rope in hand");
    assert!(world.get::<Making>(crafter).is_none(), "and the craft is over");
}

#[test]
fn a_craft_that_lost_its_components_makes_nothing_and_takes_nothing() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let names = names_for(&[(7, "rag"), (11, "rope")]);
    let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(7), 1);
    world.entity_mut(rag).remove::<cdda_sim::Position>();
    world
        .entity_mut(rag)
        .insert(cdda_sim::creature::Carried(crafter));
    let mut recipe = recipe(100, "rag", 2);
    recipe.result = "rope".to_owned();
    // Started with two, then one was dropped.
    assert_eq!(
        cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        Craft::MissingComponents,
        "this world only ever had one rag, so the craft never starts"
    );
    assert_eq!(
        cdda_sim::crafting::finish_craft(&mut world, crafter, &names),
        None,
        "and finishing nothing makes nothing"
    );
    let carried = cdda_sim::inventory::carried_by_id(&mut world, crafter, |kind| names.name_of(kind));
    assert_eq!(carried.get("rag"), Some(&1), "the rag is still there");
}
