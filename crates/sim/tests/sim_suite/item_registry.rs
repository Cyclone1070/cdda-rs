//! The store can say which item is which.
//!
//! Reference: `item_controller`'s `itype_id` table (`item_factory.cpp`), which numbers every item type and answers in
//! both directions. Until this existed the simulation could not answer "what is in my hands" without being handed a
//! lookup, which is a second source of truth wearing a function's clothes.

#![allow(clippy::panic, clippy::expect_used)]

use bevy_ecs::prelude::*;
use cdda_sim::item_registry::{self, ItemNames};
use cdda_sim::pockets::Pocket;
use cdda_sim::{ItemTypeId, crafting};

/// Turns built kinds into the borrowed slices the registry takes.
fn kinds_as(kinds: &[(String, i64, i64, Vec<Pocket>)]) -> Vec<item_registry::Kind<'_>> {
    kinds
        .iter()
        .map(|(id, volume, weight, pockets)| item_registry::Kind {
            id,
            volume_ml: *volume,
            weight_g: *weight,
            pockets,
        })
        .collect()
}

/// A kind with one plain pocket of the given size, so a worn thing has somewhere to put things.
fn bag(id: &str, volume_ml: i64, weight_g: i64, pocket_ml: i64) -> (String, i64, i64, Vec<Pocket>) {
    (
        id.to_owned(),
        volume_ml,
        weight_g,
        if pocket_ml > 0 {
            vec![Pocket {
                capacity_ml: i32::try_from(pocket_ml).expect("a pocket that fits in an i32"),
                max_weight_g: 0,
                max_item_ml: None,
                holster: false,
            }]
        } else {
            Vec::new()
        },
    )
}



#[test]
fn a_kind_can_be_looked_up_by_number_and_by_name() {
    let mut world = World::new();
    let kinds = [bag("rag", 250, 100, 0), bag("rope", 500, 200, 0), bag("rock", 1_000, 5_000, 0)];
    item_registry::set_item_types(&mut world, &kinds_as(&kinds));

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
    let kinds = [bag("rag", 250, 100, 0)];
    item_registry::set_item_types(&mut world, &kinds_as(&kinds));
    let kinds = [bag("rope", 500, 200, 0)];
    item_registry::set_item_types(&mut world, &kinds_as(&kinds));
    assert_eq!(item_registry::id_of(&mut world, "rag"), None, "a second load is not a merge");
    assert_eq!(item_registry::id_of(&mut world, "rope"), Some(ItemTypeId(1)));
}

#[test]
fn a_craft_can_be_finished_without_a_caller_supplying_the_item_table() {
    let mut world = World::new();
    let kinds = [bag("rag", 250, 100, 0), bag("rope", 500, 200, 0)];
    item_registry::set_item_types(&mut world, &kinds_as(&kinds));

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
        batch_rscale: 0.0,
        batch_rsize: 0,
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
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
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
    assert_eq!(carried.get("rag"), None, "both rags were used up");
    assert_eq!(carried.get("rope"), None, "the rope fits no pocket this survivor has");
    assert_eq!(
        cdda_sim::items_at(&mut world, (0, 0)).len(),
        1,
        "so it lies on the square the survivor stands on"
    );
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
    let kinds = [bag("jerrycan", 4_000, 2_000, 0), bag("coat", 1_000, 1_000, 6_000)];
    item_registry::set_item_types(&mut world, &kinds_as(&kinds));

    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    give_moves(&mut world, survivor, 500);
    // A coat with six litres of pockets: without one there is nowhere to put a four-litre can.
    let coat = cdda_sim::drop_item(&mut world, (1, 0), ItemTypeId(2), 1);
    world.entity_mut(coat).remove::<cdda_sim::Position>();
    world
        .entity_mut(coat)
        .insert(cdda_sim::creature::WornBy(survivor));
    let can = cdda_sim::drop_item(&mut world, (1, 0), ItemTypeId(1), 1);

    assert_eq!(pick_up_in_store(&mut world, survivor, can), Picked::Taken);
    assert_eq!(
        moves_left(&world, survivor),
        500 - 300,
        "a hundred for the handling and two hundred for four litres"
    );
}

/// A creature can carry thirteen kilograms and not a gram more, plus four a point of strength.
///
/// Reference: `Creature::weight_capacity` (`creature.cpp:3246-3268`) and `Character::weight_capacity`
/// (`character.cpp:3353-3361`). The size multipliers are the part a hand-written test would most easily get wrong, so
/// all five are checked rather than the medium one.
#[test]
fn capacity_follows_size_and_strength() {
    use cdda_sim::CreatureSize;
    use cdda_sim::creature::{Strength, carrying_capacity, spawn_creature};

    let capacity = |size: CreatureSize, strength: Option<i32>| {
        let mut world = World::new();
        let creature = spawn_creature(&mut world, "a thing", (0, 0, 0));
        world
            .entity_mut(creature)
            .insert(cdda_sim::creature::Creature {
                name: "a thing".to_owned(),
                size,
            });
        if let Some(base) = strength {
            world.entity_mut(creature).insert(Strength { base });
        }
        carrying_capacity(&world, creature)
    };

    assert_eq!(capacity(CreatureSize::Tiny, None), 3_250, "a cat carries a quarter");
    assert_eq!(capacity(CreatureSize::Small, None), 6_500, "a dog half");
    assert_eq!(capacity(CreatureSize::Medium, None), 13_000, "a person thirteen kilos");
    assert_eq!(capacity(CreatureSize::Large, None), 26_000, "a cow double");
    assert_eq!(capacity(CreatureSize::Huge, None), 52_000, "and something huge quadruple");
    assert_eq!(
        capacity(CreatureSize::Medium, Some(8)),
        45_000,
        "and four kilos a point of strength on top: eight is thirty-two kilos"
    );
}

/// A survivor cannot carry the world.
///
/// Reference: `Character::can_pickup` against `weight_capacity`. The refusal has to come **before** the moves are spent:
/// a pickup that charged you and then dropped the thing would be worse than one that refused, because the turn would
/// be gone with nothing to show for it.
#[test]
fn too_heavy_is_refused_and_costs_nothing() {
    use cdda_core_types::level::Level;
    use cdda_sim::creature::{Picked, Strength, give_moves, moves_left, pick_up_in_store, spawn_creature};
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
    // An anvil: fifty kilos, and a survivor can carry forty-five.
    let kinds = [bag("anvil", 20_000, 50_000, 0)];
    item_registry::set_item_types(&mut world, &kinds_as(&kinds));

    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    world.entity_mut(survivor).insert(Strength { base: 8 });
    give_moves(&mut world, survivor, 500);
    let anvil = cdda_sim::drop_item(&mut world, (1, 0), ItemTypeId(1), 1);

    assert_eq!(
        pick_up_in_store(&mut world, survivor, anvil),
        Picked::TooHeavy,
        "fifty kilos against forty-five"
    );
    assert_eq!(moves_left(&world, survivor), 500, "and the turn was not spent");
    assert_eq!(
        cdda_sim::items_at(&mut world, (1, 0)).len(),
        1,
        "the anvil is still on the floor where it belongs"
    );
}

/// A craft started in the morning is finished by the turn loop, and the result is in hand.
///
/// This is the wire the last two rounds left unwired: `finish_activity` had a `craft` branch that only cleared the
/// marker, because the lookups the finish needed lived in its callers. With the registry in the store, the loop closes
/// the loop by itself.
#[test]
fn the_turn_loop_finishes_what_it_started() {
    use cdda_core_types::level::Level;
    use cdda_sim::creature::{Carried, set_speed, spawn_creature};
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
    let kinds = [bag("rag", 250, 100, 0), bag("rope", 500, 200, 0)];
    item_registry::set_item_types(&mut world, &kinds_as(&kinds));

    let crafter = spawn_creature(&mut world, "the survivor", (0, 0, 0));
    set_speed(&mut world, crafter, 100, 0);
    for _ in 0..2 {
        let rag = cdda_sim::drop_item(&mut world, (0, 0), ItemTypeId(1), 1);
        world.entity_mut(rag).remove::<cdda_sim::Position>();
        world.entity_mut(rag).insert(Carried(crafter));
    }
    let names = item_registry::snapshot(&mut world);
    let recipe = cdda_content::recipes::Recipe {
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
        time_moves: 200,
        batch_rscale: 0.0,
        batch_rsize: 0,
    };
    cdda_sim::recipes_known::learn(&mut world, crafter, &recipe.id);
    assert_eq!(
        crafting::begin_craft_from_carried(&mut world, crafter, &recipe, 1, &names),
        crafting::Craft::Started
    );

    cdda_sim::turn::run_turn(&mut world);
    cdda_sim::turn::run_turn(&mut world);

    let carried = cdda_sim::inventory::carried_by_id(&mut world, crafter, |kind| {
        names.name_of(kind)
    });
    assert_eq!(carried.get("rag"), None, "the loop used both rags");
    assert_eq!(
        cdda_sim::items_at(&mut world, (0, 0)).len(),
        1,
        "and put the rope it made on the floor, since the survivor has no pockets"
    );
    assert!(world.get::<crafting::Making>(crafter).is_none(), "nothing is still being made");
}

/// What you are wearing is where things go.
///
/// Reference: `Character::volume_capacity` (`character.cpp:8883-8889`) sums the capacity of what is worn, and
/// `can_pickVolume` refuses what will not fit. The pair matters: a port that only counted weight would let a survivor put
/// a jerrycan in a t-shirt pocket.
#[test]
fn you_can_only_carry_what_you_have_room_for() {
    use cdda_core_types::level::Level;
    use cdda_sim::creature::{
        Picked, WornBy, give_moves, pick_up_in_store, spawn_creature, volume_capacity,
    };
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
    let kinds = [bag("jerrycan", 3_000, 1_000, 0), bag("backpack", 2_000, 500, 5_000)];
    item_registry::set_item_types(&mut world, &kinds_as(&kinds));
    let names = item_registry::snapshot(&mut world);

    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    give_moves(&mut world, survivor, 5_000);

    assert_eq!(
        volume_capacity(&mut world, survivor, &names),
        0,
        "a survivor wearing nothing has no room at all"
    );
    let can = cdda_sim::drop_item(&mut world, (1, 0), ItemTypeId(1), 1);
    assert_eq!(
        pick_up_in_store(&mut world, survivor, can),
        Picked::TooBig,
        "three litres into no pockets"
    );

    // Now with a backpack: five litres of room, which takes the three-litre can.
    let pack = cdda_sim::drop_item(&mut world, (1, 0), ItemTypeId(2), 1);
    world.entity_mut(pack).remove::<cdda_sim::Position>();
    world.entity_mut(pack).insert(WornBy(survivor));
    assert_eq!(volume_capacity(&mut world, survivor, &names), 5_000);
    assert_eq!(pick_up_in_store(&mut world, survivor, can), Picked::Taken);
    assert_eq!(
        cdda_sim::inventory::volume_carried(&mut world, survivor, &names),
        3_000,
        "three of the five litres are now taken"
    );
}

/// Room is asked pocket by pocket, not added up.
///
/// A survivor wearing a hundred small pockets and one big one: the big one takes the thing, and a thing too big for any
/// pocket is refused **even though the total room is plenty**. A port that compared totals would let a rifle slide into
/// twenty pockets at once, which is the mistake this test exists to catch.
#[test]
fn a_thing_needs_one_pocket_that_takes_it_and_not_a_total() {
    use cdda_core_types::level::Level;
    use cdda_sim::creature::{Picked, WornBy, can_stash, give_moves, pick_up_in_store, spawn_creature};
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

    // A vest of twenty small pockets, a hundred millilitres each: two litres of room in total, and no single pocket
    // that takes more than a hundred millilitres.
    let small = Pocket {
        capacity_ml: 100,
        max_weight_g: 0,
        max_item_ml: Some(100),
        holster: false,
    };
    let wanted = Pocket {
        capacity_ml: 4_000,
        max_weight_g: 0,
        max_item_ml: None,
        holster: false,
    };
    let kinds = [
        (String::from("rifle"), 3_000, 4_000, Vec::new()),
        (String::from("vest"), 1_000, 1_000, vec![small; 20]),
        (String::from("holster_rig"), 1_000, 1_000, vec![wanted]),
    ];
    item_registry::set_item_types(&mut world, &kinds_as(&kinds));
    let names = item_registry::snapshot(&mut world);

    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    give_moves(&mut world, survivor, 5_000);
    let vest = cdda_sim::drop_item(&mut world, (1, 0), ItemTypeId(2), 1);
    world.entity_mut(vest).remove::<cdda_sim::Position>();
    world.entity_mut(vest).insert(WornBy(survivor));
    let rifle = cdda_sim::drop_item(&mut world, (1, 0), ItemTypeId(1), 1);

    assert_eq!(
        can_stash(&mut world, survivor, &names, 3_000, 4_000),
        None,
        "two litres of room in twenty pockets still cannot hold a rifle"
    );
    assert_eq!(
        pick_up_in_store(&mut world, survivor, rifle),
        Picked::TooBig,
        "and the pickup says so"
    );

    // One pocket that takes it, and the same rifle goes in.
    let rig = cdda_sim::drop_item(&mut world, (1, 0), ItemTypeId(3), 1);
    world.entity_mut(rig).remove::<cdda_sim::Position>();
    world.entity_mut(rig).insert(WornBy(survivor));
    assert!(
        can_stash(&mut world, survivor, &names, 3_000, 4_000).is_some(),
        "the rig's one big pocket takes it"
    );
    assert_eq!(pick_up_in_store(&mut world, survivor, rifle), Picked::Taken);
}

/// A pocket fills up, and a full pocket is not made roomier by the other pockets.
///
/// Reference: an item lives inside an `item_pocket`, so the second bottle does not fit where the first one is. The
/// arithmetic that proves this matters: two pockets of a litre each hold two six-hundred-millilitre bottles and not
/// three, even though the survivor has two litres of room and only 1.8 litres carried. A port that added the total up
/// would let the third bottle in and be wrong in a way nothing would notice until a survivor carried six bottles in one
/// pocket.
#[test]
fn a_pocket_fills_up() {
    use cdda_core_types::level::Level;
    use cdda_sim::creature::{
        InPocket, Picked, WornBy, give_moves, pick_up_in_store, pocket_contents, spawn_creature,
    };
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

    let one_litre = Pocket {
        capacity_ml: 1_000,
        max_weight_g: 0,
        max_item_ml: None,
        holster: false,
    };
    let kinds = [
        (String::from("bottle"), 600, 600, Vec::new()),
        (String::from("two_pocket_vest"), 1_000, 1_000, vec![one_litre; 2]),
    ];
    item_registry::set_item_types(&mut world, &kinds_as(&kinds));
    let names = item_registry::snapshot(&mut world);

    let survivor = spawn_creature(&mut world, "the survivor", (1, 0, 0));
    give_moves(&mut world, survivor, 9_000);
    let vest = cdda_sim::drop_item(&mut world, (1, 0), ItemTypeId(2), 1);
    world.entity_mut(vest).remove::<cdda_sim::Position>();
    world.entity_mut(vest).insert(WornBy(survivor));

    let first_bottle = cdda_sim::drop_item(&mut world, (1, 0), ItemTypeId(1), 1);
    let second_bottle = cdda_sim::drop_item(&mut world, (1, 0), ItemTypeId(1), 1);
    let third_bottle = cdda_sim::drop_item(&mut world, (1, 0), ItemTypeId(1), 1);
    assert_eq!(pick_up_in_store(&mut world, survivor, first_bottle), Picked::Taken);
    assert_eq!(pick_up_in_store(&mut world, survivor, second_bottle), Picked::Taken);
    assert_eq!(
        pick_up_in_store(&mut world, survivor, third_bottle),
        Picked::TooBig,
        "two pockets of a litre, two bottles of six hundred: the third has nowhere to go"
    );

    let filled = pocket_contents(&mut world, survivor, &names);
    assert_eq!(filled.len(), 2, "both pockets are taken");
    assert!(
        filled.values().all(|counted| counted.count == 1 && counted.volume_ml == 600),
        "one bottle in each, and neither pocket is over its litre"
    );
    let first = world
        .get::<InPocket>(first_bottle)
        .copied()
        .expect("the first bottle is in a pocket");
    let second = world
        .get::<InPocket>(second_bottle)
        .copied()
        .expect("and so is the second");
    assert_ne!(
        (first.worn, first.index),
        (second.worn, second.index),
        "in different pockets, because one pocket cannot hold two of them"
    );
}
