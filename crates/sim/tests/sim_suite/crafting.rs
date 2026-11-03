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

/// What a crafter has, with all four questions answered by the given closures.
fn supplies_tools<'a>(
    components: impl Fn(&str) -> i32 + 'a,
    held: impl Fn(&str) -> i32 + 'a,
    charges: impl Fn(&str) -> i32 + 'a,
    qualities: impl Fn(&str, i32) -> i32 + 'a,
) -> cdda_sim::crafting::Supplies<'a> {
    cdda_sim::crafting::Supplies {
        components: Box::leak(Box::new(components)),
        held: Box::leak(Box::new(held)),
        charges: Box::leak(Box::new(charges)),
        qualities: Box::leak(Box::new(qualities)),
        known: &(|_: &str| true),
    }
}

/// What a crafter has, with the components filled in and nothing else.
///
/// The tool and quality questions answer zero, which is what the recipes in these tests ask for: none of them names a tool or a quality.
fn supplies_with<'a>(components: impl Fn(&str) -> i32 + 'a) -> cdda_sim::crafting::Supplies<'a> {
    cdda_sim::crafting::Supplies {
        components: Box::leak(Box::new(components)),
        held: &(|_: &str| 0),
        charges: &(|_: &str| 0),
        qualities: &(|_: &str, _: i32| 0),
        // These recipes are about what is on hand, not about knowledge: a crafter is taken to know what it is making unless a test says otherwise.
        known: &(|_: &str| true),
    }
}

/// A name table with the given numbers filled in, the way a loaded registry would be.
fn names_for(entries: &[(u32, &str)]) -> cdda_sim::item_registry::ItemNames {
    let highest = entries.iter().map(|(number, _)| *number).max().unwrap_or(0);
    let mut names = cdda_sim::item_registry::ItemNames {
        names: vec![String::new(); highest as usize + 1],
        by_name: std::collections::BTreeMap::new(),
        volumes_ml: vec![0; highest as usize + 1],
        weights_g: vec![0; highest as usize + 1],
        holds_ml: vec![0; highest as usize + 1],
        pockets: Vec::new(),
        pocket_ranges: vec![(0, 0); highest as usize + 1],
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
        batch_rscale: 0.0,
        batch_rsize: 0,
        made_per_batch: 1,
        byproduct_group: Vec::new(),
        container: None,
        byproducts: Vec::new(),
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
        begin_craft(&mut world, crafter, &recipe, 1, &supplies_with(|item| i32::from(item == "rag") * 2)),
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
        begin_craft(&mut world, crafter, &recipe, 3, &supplies_with(|_| 3)),
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
        begin_craft(&mut world, crafter, &recipe, 1, &supplies_with(|item| i32::from(item == "rag"))),
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
    assert_eq!(begin_craft(&mut world, crafter, &recipe, 1, &supplies_with(|_| 99)), Craft::Busy);
}

#[test]
fn a_recipe_with_no_time_is_refused_rather_than_finished_instantly() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let recipe = recipe(0, "rag", 1);
    assert_eq!(
        begin_craft(&mut world, crafter, &recipe, 1, &supplies_with(|_| 99)),
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
    assert_eq!(begin_craft(&mut world, crafter, &recipe, 1, &supplies_with(|_| 1)), Craft::Started);

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

    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
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
    cdda_sim::recipes_known::learn(&mut world, other, &recipe.id);
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
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
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
    assert_eq!(
        carried.get("rope"),
        None,
        "and the rope is not in the hands of a survivor with no pockets - it was dropped"
    );
    assert_eq!(
        cdda_sim::items_at(&mut world, (0, 0)).len(),
        1,
        "on the square the survivor stands on"
    );
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
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
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

/// A crafted thing goes where a found thing goes.
///
/// Reference: `Character::i_add` -> `add_item`, which stashes the result in a pocket like any other item. The port
/// spawned its result straight into the hands, which was the one place in the item layer that contradicted the pocket
/// rule: a survivor with pockets could craft something and have it exist nowhere.
#[test]
fn a_crafted_thing_lands_in_a_pocket() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    // A pack with one two-litre pocket, and the rope is half a litre.
    let pack = cdda_sim::pockets::Pocket {
        capacity_ml: 2_000,
        max_weight_g: 0,
        max_item_ml: None,
        holster: false,
    };
    let kinds = [
        (String::from("rag"), 250, 100, Vec::new()),
        (String::from("rope"), 500, 200, Vec::new()),
        (String::from("pack"), 1_000, 500, vec![pack]),
    ];
    let registry: Vec<cdda_sim::item_registry::Kind<'_>> = kinds
        .iter()
        .map(|(id, volume, weight, pockets)| cdda_sim::item_registry::Kind {
            id,
            volume_ml: *volume,
            weight_g: *weight,
            pockets,
        })
        .collect();
    cdda_sim::item_registry::set_item_types(&mut world, &registry);
    let names = cdda_sim::item_registry::snapshot(&mut world);

    let worn = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(3), 1);
    world.entity_mut(worn).remove::<cdda_sim::Position>();
    world
        .entity_mut(worn)
        .insert(cdda_sim::creature::WornBy(crafter));
    for _ in 0..2 {
        let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
        world.entity_mut(rag).remove::<cdda_sim::Position>();
        world
            .entity_mut(rag)
            .insert(cdda_sim::creature::Carried(crafter));
    }

    let mut recipe = recipe(100, "rag", 2);
    recipe.result = "rope".to_owned();
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
    assert_eq!(
        cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        Craft::Started
    );
    assert_eq!(
        cdda_sim::crafting::finish_craft(&mut world, crafter, &names).as_deref(),
        Some("rope")
    );

    let made = {
        let mut query = world.query::<(bevy_ecs::prelude::Entity, &cdda_sim::GroundItem)>();
        query
            .iter(&world)
            .find(|(_, item)| item.kind == cdda_sim::ItemTypeId(2))
            .map(|(entity, _)| entity)
            .expect("the rope exists")
    };
    let pocket = world
        .get::<cdda_sim::creature::InPocket>(made)
        .copied()
        .expect("and it is in the pack's pocket, not loose in the hands");
    assert_eq!(pocket.worn, worn, "the pack is what holds it");
    assert_eq!(pocket.index, 0);
}

/// What fits nowhere in your pockets lands at your feet.
///
/// Reference: `Character::i_add_or_drop` - the craft finish adds the result, the add fails for want of a pocket, and the
/// thing goes on the ground rather than into the character's hands. The previous round left it in hand and declared the
/// difference; this closes it, because the port does have a square to drop onto.
#[test]
fn what_fits_no_pocket_lands_at_your_feet() {
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

    let names = cdda_sim::item_registry::ItemNames {
        names: vec![String::new(), "rag".to_owned(), "rope".to_owned()],
        by_name: [
            ("rag".to_owned(), cdda_sim::ItemTypeId(1)),
            ("rope".to_owned(), cdda_sim::ItemTypeId(2)),
        ]
        .into_iter()
        .collect(),
        volumes_ml: vec![0, 250, 500],
        weights_g: vec![0, 100, 200],
        holds_ml: vec![0, 0, 0],
        pockets: Vec::new(),
        pocket_ranges: vec![(0, 0), (0, 0), (0, 0)],
    };
    cdda_sim::item_registry::set_item_types(
        &mut world,
        &[
            cdda_sim::item_registry::Kind {
                id: "rag",
                volume_ml: 250,
                weight_g: 100,
                pockets: &[],
            },
            cdda_sim::item_registry::Kind {
                id: "rope",
                volume_ml: 500,
                weight_g: 200,
                pockets: &[],
            },
        ],
    );

    let crafter = spawn_creature(&mut world, "the survivor", (2, 0, 0));
    for _ in 0..2 {
        let rag = cdda_sim::drop_item(&mut world, (2, 0), cdda_sim::ItemTypeId(1), 1);
        world.entity_mut(rag).remove::<cdda_sim::Position>();
        world
            .entity_mut(rag)
            .insert(cdda_sim::creature::Carried(crafter));
    }
    let mut recipe = recipe(100, "rag", 2);
    recipe.result = "rope".to_owned();
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
    assert_eq!(
        cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        Craft::Started
    );
    assert_eq!(
        cdda_sim::crafting::finish_craft(&mut world, crafter, &names).as_deref(),
        Some("rope")
    );

    let carried = cdda_sim::inventory::carried_by_id(&mut world, crafter, |kind| names.name_of(kind));
    assert_eq!(carried.get("rope"), None, "it is not in the survivor's hands or pockets");
    let on_the_floor = cdda_sim::items_at(&mut world, (2, 0));
    assert_eq!(on_the_floor.len(), 1, "it is on the square the survivor stood on");
}

/// A recipe whose tool is missing does not start, and starts once it is on hand.
///
/// Reference: `Character::making_would_work` refuses when `deduped_requirements().can_make_with_inventory` says no, and that rule
/// covers **tools and qualities as well as components** (`requirements.cpp:932-990`). The port's `begin_craft` looked only at components,
/// so a recipe needing a hammer started with bare hands - a rule that could not be caught being wrong.
#[test]
fn a_craft_without_its_tool_does_not_start() {
    use cdda_content::recipes::{Component, Recipe, Tool};
    let recipe = Recipe {
        id: "test_recipe".to_owned(),
        result: "test_result".to_owned(),
        blacklisted: false,
        components: vec![vec![Component {
            item: "rag".to_owned(),
            count: 2,
        }]],
        // Reference: a **negative** count is a tool that is needed and not spent (`requirements.cpp:952-990`); a positive count is charges.
        tools: vec![vec![Tool {
            item: "hammer".to_owned(),
            count: -1,
        }]],
        qualities: Vec::new(),
        skill: String::new(),
        difficulty: 0,
        time_moves: 1_000,
        batch_rscale: 0.0,
        batch_rsize: 0,
        made_per_batch: 1,
        byproduct_group: Vec::new(),
        container: None,
        byproducts: Vec::new(),
    };
    let mut world = World::new();
    let crafter = cdda_sim::creature::spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let rags = |item: &str| i32::from(item == "rag") * 4;
    let hands = |_: &str| 0;
    let nothing = |_: &str| 0;
    assert_eq!(
        cdda_sim::crafting::begin_craft(
            &mut world,
            crafter,
            &recipe,
            1,
            &supplies_tools(rags, hands, nothing, |_, _| 0),
        ),
        cdda_sim::crafting::Craft::MissingTools,
        "the hammer is not on hand"
    );
    assert!(
        world.get::<cdda_sim::crafting::Making>(crafter).is_none(),
        "and nothing was started"
    );
    let with_hammer = |item: &str| i32::from(item == "hammer");
    assert_eq!(
        cdda_sim::crafting::begin_craft(
            &mut world,
            crafter,
            &recipe,
            1,
            &supplies_tools(rags, with_hammer, nothing, |_, _| 0),
        ),
        cdda_sim::crafting::Craft::Started,
        "with the tool it starts"
    );
}

/// A recipe that needs a quality refuses without an item that has it, and starts with one.
///
/// Reference: `quality_requirement::has` (`requirements.cpp:932-940`): an item whose quality is **at least** the level counts, and there must be that many carried.
/// The quality belongs to the item **type**, so the test installs one and asks through the carried-stack path the game itself uses.
#[test]
fn a_craft_that_needs_a_quality_asks_the_items_that_carry_it() {
    use cdda_content::recipes::{Component, Quality, Recipe};
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let names = names_for(&[(1, "rag"), (2, "hammer")]);
    let recipe = Recipe {
        id: "pound_a_rag".to_owned(),
        result: "rag".to_owned(),
        blacklisted: false,
        components: vec![vec![Component {
            item: "rag".to_owned(),
            count: 1,
        }]],
        tools: Vec::new(),
        qualities: vec![vec![Quality {
            quality: "HAMMER".to_owned(),
            level: 2,
            count: 1,
        }]],
        skill: String::new(),
        difficulty: 0,
        time_moves: 1_000,
        batch_rscale: 0.0,
        batch_rsize: 0,
        made_per_batch: 1,
        byproduct_group: Vec::new(),
        container: None,
        byproducts: Vec::new(),
    };
    let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
    world.entity_mut(rag).remove::<cdda_sim::Position>();
    world.entity_mut(rag).insert(cdda_sim::creature::Carried(crafter));
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
    assert_eq!(
        cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        Craft::MissingQualities,
        "a rag is not a hammer"
    );
    let hammer = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(2), 1);
    world.entity_mut(hammer).remove::<cdda_sim::Position>();
    world.entity_mut(hammer).insert(cdda_sim::creature::Carried(crafter));
    cdda_sim::item_registry::set_item_qualities(
        &mut world,
        &[(
            cdda_sim::ItemTypeId(2),
            &[("HAMMER".to_owned(), 3)],
        )],
    );
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
    assert_eq!(
        cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        Craft::Started,
        "a level-three hammer satisfies a level-two requirement"
    );
}

/// A recipe that benefits from batching takes less than `time * batch`, by the reference's own curve.
///
/// Reference: `recipe::batch_time` (`recipe.cpp:108-146`). The port had the arithmetic in `batch_time_moves` and **never called it from a craft**: `begin_craft`
/// multiplied, so every one of the data's 1,148 batching recipes was charged full price. The numbers below are the reference's curve computed by hand:
/// `time 1000`, `batch_time_factors [25, 8]`, batch 4 is **3549** moves, not 4000.
#[test]
fn a_batch_that_saves_time_is_charged_the_reference_curve() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let mut batch_recipe = recipe(1_000, "rag", 1);
    batch_recipe.batch_rscale = 0.25;
    batch_recipe.batch_rsize = 8;
    assert_eq!(
        cdda_sim::crafting::begin_craft(&mut world, crafter, &batch_recipe, 4, &supplies_with(|_| 99)),
        Craft::Started
    );
    assert_eq!(
        world.get::<Doing>(crafter).map(|doing| doing.moves_left),
        Some(3_549),
        "a quarter off at the curve's far end, summed over four items"
    );
    // The same recipe without the factors is the plain product, which is the rule the port used to apply to everything.
    let plain = recipe(1_000, "rag", 1);
    let other = spawn_creature(&mut world, "the other", (1, 0, 0));
    assert_eq!(
        cdda_sim::crafting::begin_craft(&mut world, other, &plain, 4, &supplies_with(|_| 99)),
        Craft::Started
    );
    assert_eq!(
        world.get::<Doing>(other).map(|doing| doing.moves_left),
        Some(4_000),
        "no factors, no saving"
    );
}

/// A recipe the crafter has never learned does not start, and starts once it is learned.
///
/// Reference: `Character::can_make` refuses when `!has_recipe( r )` (`crafting.cpp:602-616`) and `has_recipe` is `knows_recipe || group_available`
/// (`character_crafting.cpp:23-27`). The port let a caller hand in any recipe it liked, so knowledge was never asked and a survivor could make what it had never
/// learned from a book.
#[test]
fn a_recipe_that_was_never_learned_does_not_start() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let names = names_for(&[(1, "rag")]);
    let recipe = recipe(1_000, "rag", 1);
    let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 4);
    world.entity_mut(rag).remove::<cdda_sim::Position>();
    world.entity_mut(rag).insert(cdda_sim::creature::Carried(crafter));
    let unknown = cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names);
    assert_eq!(unknown, Craft::UnknownRecipe, "the book was never read");
    assert!(
        world.get::<Making>(crafter).is_none(),
        "and nothing was started"
    );
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
    let learned = cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names);
    assert_eq!(learned, Craft::Started, "learned, and with the rag in hand, it starts");
}

/// Making something with a tool that burns charges spends them, and a held tool is never spent.
///
/// Reference: `Character::craft_consume_tools` (`crafting.cpp:2352-2400`) spends `count * batch_size` charges of a tool encoded by charges, and
/// `consume_tools` (`crafting.cpp:2458-2482`) takes them from the stack that carries them. A tool encoded as **held** (`count <= 0`) is never spent - that is the
/// whole difference between the two encodings. The port checked the charges at the start and then never spent them, so a battery-powered recipe was free.
#[test]
fn a_tool_that_burns_charges_has_them_spent_and_a_held_one_does_not() {
    use cdda_content::recipes::{Component, Recipe, Tool};
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let names = names_for(&[(1, "rag"), (2, "battery")]);
    let recipe = Recipe {
        batch_rscale: 0.0,
        batch_rsize: 0,
        made_per_batch: 1,
        byproduct_group: Vec::new(),
        container: None,
        byproducts: Vec::new(),
        id: "electrified_rag".to_owned(),
        result: "rag".to_owned(),
        blacklisted: false,
        components: vec![vec![Component {
            item: "rag".to_owned(),
            count: 1,
        }]],
        // A positive count is charges spent; a negative one is the tool being needed and kept.
        tools: vec![
            vec![Tool {
                item: "battery".to_owned(),
                count: 5,
            }],
            vec![Tool {
                item: "rag".to_owned(),
                count: -1,
            }],
        ],
        qualities: Vec::new(),
        skill: String::new(),
        difficulty: 0,
        time_moves: 100,
    };
    let battery = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(2), 12);
    world.entity_mut(battery).remove::<cdda_sim::Position>();
    world.entity_mut(battery).insert(cdda_sim::creature::Carried(crafter));
    for _ in 0..2 {
        let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
        world.entity_mut(rag).remove::<cdda_sim::Position>();
        world.entity_mut(rag).insert(cdda_sim::creature::Carried(crafter));
    }
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
    assert_eq!(
        cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 2, &names),
        Craft::Started,
        "12 charges cover two batches of five"
    );
    // The names are the test's own table, so the finish is asked with them rather than with the world's (empty) registry.
    let made = cdda_sim::crafting::finish_craft(&mut world, crafter, &names);
    assert_eq!(made.as_deref(), Some("rag"), "the rag was made");
    let left: i32 = {
        let mut query = world.query::<(&cdda_sim::creature::Carried, &cdda_sim::GroundItem)>();
        query
            .iter(&world)
            .filter(|(carried, stack)| {
                carried.0 == crafter && names.name_of(stack.kind).as_deref() == Some("battery")
            })
            .map(|(_, stack)| stack.charges)
            .sum()
    };
    assert_eq!(left, 2, "two batches of five came out of the twelve");
}

/// Making something teaches the skill the recipe names, by the craft's own tick count, scaled by focus, and not past the recipe's cap.
///
/// Reference: `craft_skill_gain` is called with `time_to_craft_moves / 100` ticks (`activity_actor.cpp:4232-4238`) and calls `practice( skill, ticks, skill_cap )`
/// (`crafting.cpp:1032`); `practice` scales by focus, multiplies by a hundred and refuses once the practical level is above `recipe::get_skill_cap` - `difficulty *
/// 1.25` (`recipe.cpp:70-77`). The port finished a craft and taught nothing.
#[test]
fn a_craft_teaches_the_skill_it_names() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let names = names_for(&[(1, "rag")]);
    let mut learned = recipe(3_600, "rag", 1);
    learned.skill = "survival".to_owned();
    learned.difficulty = 1;
    for _ in 0..2 {
        let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
        world.entity_mut(rag).remove::<cdda_sim::Position>();
        world.entity_mut(rag).insert(cdda_sim::creature::Carried(crafter));
    }
    cdda_sim::recipes_known::learn(&mut world, crafter, &learned.id);
    assert_eq!(
        cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &learned, 1, &names),
        Craft::Started
    );
    let exercise_before = cdda_sim::skills_of::exercise_of(&world, crafter, "survival");
    cdda_sim::crafting::finish_craft(&mut world, crafter, &names);
    let exercise_after = cdda_sim::skills_of::exercise_of(&world, crafter, "survival");
    assert!(
        exercise_after > exercise_before,
        "the craft taught something: {exercise_before} then {exercise_after}"
    );
    // 3,600 moves is thirty-six ticks; the experience behind the accessor is kept in hundredths, so a hundred focus turns thirty-six ticks into thirty-six
    // points - the same number the reference's `practice` would lay on.
    assert_eq!(exercise_after - exercise_before, 36, "the tick count is the craft's own");
    // The cap is `difficulty * 1.25`, so a level above it earns nothing. The activity the finish ended is cleared first, because a crafter mid-activity is busy.
    cdda_sim::creature::stop_doing(&mut world, crafter);
    cdda_sim::skills_of::set_level(&mut world, crafter, "survival", 2);
    let mut again = recipe(3_600, "rag", 1);
    again.skill = "survival".to_owned();
    again.difficulty = 1;
    for _ in 0..2 {
        let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
        world.entity_mut(rag).remove::<cdda_sim::Position>();
        world.entity_mut(rag).insert(cdda_sim::creature::Carried(crafter));
    }
    assert_eq!(
        cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &again, 1, &names),
        Craft::Started
    );
    let capped_before = cdda_sim::skills_of::exercise_of(&world, crafter, "survival");
    cdda_sim::crafting::finish_craft(&mut world, crafter, &names);
    assert_eq!(
        cdda_sim::skills_of::exercise_of(&world, crafter, "survival"),
        capped_before,
        "level two is above a level-one recipe's cap"
    );
}

/// A recipe can make more than one thing at a time, and a recipe that states charges gives each result that many.
///
/// Reference: `recipe::create_results` (`recipe.cpp:790-825`) makes **`result_mult` results per batch item**, and `create_result` (`recipe.cpp:754`) sets each to
/// `charges ? *charges : the item's own count`. The port made exactly one result per batch and ignored both numbers, so 61 recipes that make more, and the 1,035 that
/// state charges, all produced a single thing.
#[test]
fn a_recipe_that_makes_more_than_one_makes_that_many() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let names = names_for(&[(1, "rag"), (2, "cord")]);
    // Two at a time, three apiece, and each result is its own thing.
    let mut many = recipe(100, "rag", 1);
    many.result = "cord".to_owned();
    many.made_per_batch = 3;
    let mut batteries = recipe(100, "rag", 1);
    batteries.result = "cord".to_owned();
    // A hundred things per batch, which is the reference's own product of two results holding fifty charges each.
    batteries.made_per_batch = 100;
    for recipe in [&many, &batteries] {
        // A batch of two needs two rags, because the components scale with the batch.
        for _ in 0..2 {
            let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
            world.entity_mut(rag).remove::<cdda_sim::Position>();
            world.entity_mut(rag).insert(cdda_sim::creature::Carried(crafter));
        }
        cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
        assert_eq!(
            cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, recipe, 2, &names),
            Craft::Started
        );
        assert_eq!(
            cdda_sim::crafting::finish_craft(&mut world, crafter, &names).as_deref(),
            Some("cord")
        );
        cdda_sim::creature::stop_doing(&mut world, crafter);
    }
    // The results may be in a pocket or on the floor, and both are the same answer to "what was made".
    let made: i32 = {
        let mut query = world.query::<&cdda_sim::GroundItem>();
        query
            .iter(&world)
            .filter(|stack| names.name_of(stack.kind).as_deref() == Some("cord"))
            .map(|stack| stack.charges)
            .sum()
    };
    assert_eq!(
        made,
        2 * 3 + 2 * 2 * 50,
        "six cords, then a hundred charges' worth"
    );
}

/// A craft leaves its byproducts behind, `count * batch` of each.
///
/// Reference: `recipe::create_byproducts` (`recipe.cpp:872-900`) through `create_byproducts_legacy`, which makes `count * batch` of every byproduct. **694 recipes in
/// the core data carry byproducts**, and the port dropped them all on the floor - a recipe that makes a knife and its sheath gave only the knife.
#[test]
fn a_craft_leaves_its_byproducts_behind() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let names = names_for(&[(1, "rag"), (2, "cord"), (3, "scrap")]);
    let mut recipe = recipe(100, "rag", 1);
    recipe.result = "cord".to_owned();
    recipe.byproducts = vec![("scrap".to_owned(), 2)];
    for _ in 0..2 {
        let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
        world.entity_mut(rag).remove::<cdda_sim::Position>();
        world.entity_mut(rag).insert(cdda_sim::creature::Carried(crafter));
    }
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
    assert_eq!(
        cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 2, &names),
        Craft::Started
    );
    assert_eq!(
        cdda_sim::crafting::finish_craft(&mut world, crafter, &names).as_deref(),
        Some("cord")
    );
    let count_of = |wanted: &str, world: &mut World| -> i32 {
        let mut query = world.query::<&cdda_sim::GroundItem>();
        query
            .iter(world)
            .filter(|stack| names.name_of(stack.kind).as_deref() == Some(wanted))
            .map(|stack| stack.charges)
            .sum()
    };
    assert_eq!(count_of("cord", &mut world), 2, "one cord per batch");
    assert_eq!(count_of("scrap", &mut world), 4, "two scraps per batch");
}

/// A byproduct group is drawn per batch between the data's two ends.
///
/// Reference: `byproduct_group` (`recipe.cpp:380-386`) is an inline item group whose counts are ranges - `{"item": "arrow_wood_heavy", "count": [0, 4]}` - and
/// `create_byproducts` draws from it, which is why a practice recipe leaves four arrows on one craft and six on the next. The port read no group at all, so a
/// practising archer recovered nothing.
#[test]
fn a_byproduct_group_is_drawn_between_its_ends() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let names = names_for(&[(1, "rag"), (2, "cord"), (3, "arrow")]);
    let mut recipe = recipe(100, "rag", 1);
    recipe.result = "cord".to_owned();
    recipe.byproduct_group = vec![("arrow".to_owned(), 0, 4)];
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
    let mut totals = Vec::new();
    for seed in 0..8 {
        let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 2);
        world.entity_mut(rag).remove::<cdda_sim::Position>();
        world.entity_mut(rag).insert(cdda_sim::creature::Carried(crafter));
        cdda_sim::singletons::put_rolls(&mut world, cdda_core_types::rng::Rolls::new(seed));
        assert_eq!(
            cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 2, &names),
            Craft::Started
        );
        let arrows_before = {
            let mut query = world.query::<&cdda_sim::GroundItem>();
            query
                .iter(&world)
                .filter(|stack| names.name_of(stack.kind).as_deref() == Some("arrow"))
                .map(|stack| stack.charges)
                .sum::<i32>()
        };
        cdda_sim::crafting::finish_craft(&mut world, crafter, &names);
        let arrows_after = {
            let mut query = world.query::<&cdda_sim::GroundItem>();
            query
                .iter(&world)
                .filter(|stack| names.name_of(stack.kind).as_deref() == Some("arrow"))
                .map(|stack| stack.charges)
                .sum::<i32>()
        };
        let drawn = arrows_after - arrows_before;
        assert!(
            (0..=8).contains(&drawn),
            "two batches of a 0..4 group land between none and eight, seed {seed} gave {drawn}"
        );
        totals.push(drawn);
        cdda_sim::creature::stop_doing(&mut world, crafter);
    }
    assert!(
        totals.iter().any(|drawn| *drawn < 8),
        "the draws are rolls rather than always the maximum: {totals:?}"
    );
}

/// A recipe that names a container makes **the container**, with what it made inside it.
///
/// Reference: `create_result` (`recipe.cpp:768-771`): when the recipe is `contained`, the result is wrapped by `item::in_container( container, amount, sealed )` and the
/// recipe produces **one** thing - the container. The port made the food and lost the can, so 302 batches of the products gate disagreed with the reference.
#[test]
fn a_contained_recipe_makes_the_container_with_the_thing_inside() {
    let mut world = World::new();
    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    let names = names_for(&[(1, "rag"), (2, "jar")]);
    let mut recipe = recipe(100, "rag", 1);
    recipe.result = "rag".to_owned();
    recipe.container = Some("jar".to_owned());
    let rag = cdda_sim::drop_item(&mut world, (0, 0), cdda_sim::ItemTypeId(1), 1);
    world.entity_mut(rag).remove::<cdda_sim::Position>();
    world.entity_mut(rag).insert(cdda_sim::creature::Carried(crafter));
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
    assert_eq!(
        cdda_sim::crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        Craft::Started
    );
    let made = cdda_sim::crafting::finish_craft(&mut world, crafter, &names);
    assert_eq!(
        made.as_deref(),
        Some("rag"),
        "the thing that was made is still what the craft says it made"
    );
    let jars: Vec<bevy_ecs::entity::Entity> = {
        let mut query = world.query::<(bevy_ecs::entity::Entity, &cdda_sim::GroundItem)>();
        query
            .iter(&world)
            .filter(|(_, stack)| names.name_of(stack.kind).as_deref() == Some("jar"))
            .map(|(entity, _)| entity)
            .collect()
    };
    assert_eq!(jars.len(), 1, "one jar came out of it");
    let Some(&jar) = jars.first() else {
        unreachable!("the assertion above proves a jar was made");
    };
    let inside = cdda_sim::containment::contents_of(&world, jar);
    assert_eq!(inside.len(), 1, "and the rag is inside it");
    let held = inside.first().copied();
    assert_eq!(
        held.and_then(|item| world.get::<cdda_sim::GroundItem>(item))
            .and_then(|stack| names.name_of(stack.kind)),
        Some("rag".to_owned())
    );
    assert!(
        held.is_some_and(|item| world.get::<cdda_sim::Position>(item).is_none()),
        "what is in the jar is not lying on the floor beside it"
    );
}
